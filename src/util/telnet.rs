use tokio::io::{AsyncWrite, AsyncWriteExt};

const IAC: u8 = 255; // Interpret As Command
const WILL: u8 = 251; // I will do option
const WONT: u8 = 252; // I won't do option
const DO: u8 = 253; // Please, you do option
const DONT: u8 = 254; // Please, you don't do option
const SB: u8 = 250; // Subnegotiation begin
const SE: u8 = 240; // Subnegotiation end

const ECHO: u8 = 1; // Who echoes (server will echo if we send WILL ECHO)
const SGA: u8 = 3; // Suppress Go-Ahead (interactive mode)
const TTYPE: u8 = 24; // Terminal type
const NAWS: u8 = 31; // Negotiate About Window Size
const LINEMODE: u8 = 34; // We want this OFF for char-at-a-time

#[derive(Debug)]
pub enum TelnetIn {
    /// Regular data byte
    Data(u8),
    /// Client resized terminal; cols and rows in characters
    Naws { cols: u16, rows: u16 },
}

#[derive(Debug)]
pub struct TelnetResponse {
    /// Event to be processed (if any)
    pub event: Option<TelnetIn>,
    /// IAC response bytes to send back to client (if any)
    pub response: Option<Vec<u8>>,
}

pub struct TelnetMachine {
    /// Are we in an IAC sequence?
    in_iac: bool,
    /// If in_iac, which command are we processing (WILL/WONT/DO/DONT/SB)
    in_cmd: Option<u8>,
    /// Are we inside a subnegotiation (after SB, before IAC SE)?
    in_sb: bool,
    /// If in_sb, which option are we negotiating?
    sb_opt: u8,
    /// If in_sb, buffer of data bytes collected so far
    sb_buf: Vec<u8>,
}

impl Default for TelnetMachine {
    fn default() -> Self {
        Self::new()
    }
}

impl TelnetMachine {
    pub fn new() -> Self {
        Self {
            in_iac: false,
            in_cmd: None,
            in_sb: false,
            sb_opt: 0,
            sb_buf: Vec::with_capacity(16),
        }
    }

    /// Send a sane baseline: character mode, we echo (so client stops local echo), and ask for NAWS.
    pub async fn start_negotiation<W: AsyncWrite + Unpin>(&mut self, w: &mut W) -> std::io::Result<()> {
        // Character-at-a-time: disable LINEMODE; enable SGA both directions.
        send_dont(w, LINEMODE).await?;
        send_do(w, SGA).await?;
        send_will(w, SGA).await?;

        // Server will echo (client disables local echo). We'll repaint the line ourselves.
        send_will(w, ECHO).await?;

        // Ask for window size; if client supports it, we'll get SB NAWS cols rows
        send_do(w, NAWS).await?;

        // (Optional) ask for terminal type
        // send_do(w, TTYPE).await?;

        Ok(())
    }

    /// Feed one byte; respond to negotiations and produce `TelnetIn::Data` for your editor.
    pub fn push(&mut self, b: u8) -> TelnetResponse {
        if !self.in_iac {
            if b == IAC {
                self.in_iac = true;
                return TelnetResponse {
                    event: None,
                    response: None,
                };
            }
            // Normal data byte
            return if !self.in_sb {
                TelnetResponse {
                    event: Some(TelnetIn::Data(b)),
                    response: None,
                }
            } else {
                // Inside SB: collect until IAC SE
                self.sb_buf.push(b);
                TelnetResponse {
                    event: None,
                    response: None,
                }
            };
        }

        // We are after an IAC
        self.in_iac = false;

        match b {
            IAC => {
                // Escaped 0xFF in data
                if !self.in_sb {
                    TelnetResponse {
                        event: Some(TelnetIn::Data(IAC)),
                        response: None,
                    }
                } else {
                    self.sb_buf.push(IAC);
                    TelnetResponse {
                        event: None,
                        response: None,
                    }
                }
            }
            DO | DONT | WILL | WONT => {
                self.in_cmd = Some(b);
                self.in_iac = true; // expect option next
                TelnetResponse {
                    event: None,
                    response: None,
                }
            }
            SB => {
                self.in_sb = true;
                self.sb_buf.clear();
                self.in_iac = true; // next should be the option byte
                self.in_cmd = Some(SB);
                TelnetResponse {
                    event: None,
                    response: None,
                }
            }
            SE => {
                // End subnegotiation
                if self.in_sb {
                    let opt = self.sb_opt;
                    let data = std::mem::take(&mut self.sb_buf);
                    self.in_sb = false;
                    // Handle NAWS: 4 bytes: cols_hi, cols_lo, rows_hi, rows_lo
                    if opt == NAWS && data.len() >= 4 {
                        let cols = u16::from_be_bytes([data[0], data[1]]);
                        let rows = u16::from_be_bytes([data[2], data[3]]);
                        return TelnetResponse {
                            event: Some(TelnetIn::Naws { cols, rows }),
                            response: None,
                        };
                    }
                }
                TelnetResponse {
                    event: None,
                    response: None,
                }
            }
            opt => {
                // This branch is hit when we were expecting an option byte after DO/DON'T/WILL/WON'T, or SB's option.
                if let Some(cmd) = self.in_cmd.take() {
                    let response = match cmd {
                        DO => {
                            // Client requests we WILL <opt>
                            match opt {
                                ECHO => Some(make_will(ECHO)),         // We'll echo (client stops local echo)
                                SGA => Some(make_will(SGA)),           // We'll suppress go-ahead
                                LINEMODE => Some(make_wont(LINEMODE)), // We refuse LINEMODE
                                NAWS => None,
                                _ => Some(make_wont(opt)), // We won't do any unknown options
                            }
                        }
                        DONT => {
                            // Client says DON'T <opt> (stop doing it)
                            match opt {
                                ECHO | SGA | LINEMODE => Some(make_wont(opt)),
                                _ => None,
                            }
                        }
                        WILL => {
                            // Client will do <opt>
                            match opt {
                                ECHO => Some(make_do(ECHO)),           // ok, you handle echo (we'd usually prefer we echo)
                                SGA => Some(make_do(SGA)),             // ok, you suppress go-ahead too
                                LINEMODE => Some(make_dont(LINEMODE)), // nope, please don't
                                NAWS => Some(make_do(NAWS)),           // yes, please send SB NAWS
                                TTYPE => Some(make_do(TTYPE)),         // yes, please send SB TTYPE
                                _ => Some(make_dont(opt)),
                            }
                        }
                        WONT => None,
                        SB => {
                            // This was SB option byte
                            self.sb_opt = opt;
                            return TelnetResponse {
                                event: None,
                                response: None,
                            };
                        }
                        _ => None,
                    };

                    return TelnetResponse { event: None, response };
                }

                // Unexpected lone option byte; if in SB, treat as data start
                if self.in_sb {
                    self.sb_opt = opt;
                }
                TelnetResponse {
                    event: None,
                    response: None,
                }
            }
        }
    }
}

// Helper functions to build IAC response bytes
fn make_do(opt: u8) -> Vec<u8> {
    vec![IAC, DO, opt]
}

fn make_dont(opt: u8) -> Vec<u8> {
    vec![IAC, DONT, opt]
}

fn make_will(opt: u8) -> Vec<u8> {
    vec![IAC, WILL, opt]
}

fn make_wont(opt: u8) -> Vec<u8> {
    vec![IAC, WONT, opt]
}

// Keep these for initial negotiation
async fn send3<W: AsyncWrite + Unpin>(w: &mut W, a: u8, b: u8, c: u8) -> std::io::Result<()> {
    w.write_all(&[a, b, c]).await
}

async fn send_do<W: AsyncWrite + Unpin>(w: &mut W, opt: u8) -> std::io::Result<()> {
    send3(w, IAC, DO, opt).await
}
async fn send_dont<W: AsyncWrite + Unpin>(w: &mut W, opt: u8) -> std::io::Result<()> {
    send3(w, IAC, DONT, opt).await
}
async fn send_will<W: AsyncWrite + Unpin>(w: &mut W, opt: u8) -> std::io::Result<()> {
    send3(w, IAC, WILL, opt).await
}
#[allow(unused)]
async fn send_wont<W: AsyncWrite + Unpin>(w: &mut W, opt: u8) -> std::io::Result<()> {
    send3(w, IAC, WONT, opt).await
}
