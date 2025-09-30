use tokio::io::AsyncWriteExt;
use tokio::net::tcp::OwnedWriteHalf;

const IAC: u8 = 255;        // Interpret As Command
const WILL: u8 = 251;       // I will do option
const WONT: u8 = 252;       // I won't do option
const DO: u8 = 253;         // Please, you do option
const DONT: u8 = 254;       // Please, you don't do option
const SB: u8 = 250;         // Subnegotiation begin
const SE: u8 = 240;         // Subnegotiation end

const ECHO:     u8 = 1;   // Who echoes (server will echo if we send WILL ECHO)
const SGA:      u8 = 3;   // Suppress Go-Ahead (interactive mode)
const TTYPE:    u8 = 24;  // Terminal type
const NAWS:     u8 = 31;  // Negotiate About Window Size
const LINEMODE: u8 = 34;  // We want this OFF for char-at-a-time

#[derive(Debug)]
pub enum TelnetIn {
    Data(u8),
    Naws { cols: u16, rows: u16 },
}

pub struct TelnetMachine {
    in_iac: bool,
    in_cmd: Option<u8>,
    in_sb: bool,
    sb_opt: u8,
    sb_buf: Vec<u8>,
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
    pub async fn start_negotiation(&mut self, w: &mut OwnedWriteHalf) -> std::io::Result<()> {
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
    pub async fn push(&mut self, b: u8, w: &mut OwnedWriteHalf) -> std::io::Result<Option<TelnetIn>> {
        if !self.in_iac {
            if b == IAC {
                self.in_iac = true;
                return Ok(None);
            }
            // Normal data byte
            return if !self.in_sb {
                Ok(Some(TelnetIn::Data(b)))
            } else {
                // Inside SB: collect until IAC SE
                self.sb_buf.push(b);
                Ok(None)
            }
        }

        // We are after an IAC
        self.in_iac = false;

        match b {
            IAC => {
                // Escaped 0xFF in data
                if !self.in_sb {
                    Ok(Some(TelnetIn::Data(IAC)))
                } else {
                    self.sb_buf.push(IAC);
                    Ok(None)
                }
            }
            DO | DONT | WILL | WONT => {
                self.in_cmd = Some(b);
                self.in_iac = true; // expect option next
                Ok(None)
            }
            SB => {
                self.in_sb = true;
                self.sb_buf.clear();
                self.in_iac = true; // next should be the option byte
                self.in_cmd = Some(SB);
                Ok(None)
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
                        return Ok(Some(TelnetIn::Naws { cols, rows }));
                    }
                }
                Ok(None)
            }
            opt => {
                // This branch is hit when we were expecting an option byte after DO/DON'T/WILL/WON'T, or SB's option.
                if let Some(cmd) = self.in_cmd.take() {
                    match cmd {
                        DO => { // Client requests we WILL <opt>
                            match opt {
                                ECHO => send_will(w, ECHO).await?,        // We'll echo (client stops local echo)
                                SGA  => send_will(w, SGA).await?,         // We'll suppress go-ahead
                                LINEMODE => send_wont(w, LINEMODE).await?,// We refuse LINEMODE
                                NAWS => { /* client asking us to do NAWS is unusual; ignore */ }
                                _ => send_wont(w, opt).await?,
                            }
                        }
                        DONT => { // Client says DON'T <opt> (stop doing it)
                            match opt {
                                ECHO | SGA | LINEMODE => send_wont(w, opt).await?,
                                _ => {/* ignore */},
                            }
                        }
                        WILL => { // Client will do <opt>
                            match opt {
                                ECHO => send_do(w, ECHO).await?,          // ok, you handle echo (we'd usually prefer we echo)
                                SGA  => send_do(w, SGA).await?,           // ok, you suppress go-ahead too
                                LINEMODE => send_dont(w, LINEMODE).await?,// nope, please don't
                                NAWS => send_do(w, NAWS).await?,          // yes, please send SB NAWS
                                TTYPE => send_do(w, TTYPE).await?,        // yes, please send SB TTYPE
                                _ => send_dont(w, opt).await?,
                            }
                        }
                        WONT => { // Client refuses <opt>
                            // Nothing to do; we might fall back if we relied on it
                        }
                        SB => {
                            // This was SB option byte
                            self.sb_opt = opt;
                            return Ok(None);
                        }
                        _ => {}
                    }
                    return Ok(None);
                }

                // Unexpected lone option byte; if in SB, treat as data start
                if self.in_sb {
                    self.sb_opt = opt;
                }
                Ok(None)
            }
        }
    }
}

async fn send3(w: &mut OwnedWriteHalf, a: u8, b: u8, c: u8) -> std::io::Result<()> {
    w.write_all(&[a, b, c]).await
}

async fn send_do(w: &mut OwnedWriteHalf, opt: u8) -> std::io::Result<()> {
    send3(w, IAC, DO, opt).await
}
async fn send_dont(w: &mut OwnedWriteHalf, opt: u8) -> std::io::Result<()> {
    send3(w, IAC, DONT, opt).await
}
async fn send_will(w: &mut OwnedWriteHalf, opt: u8) -> std::io::Result<()> {
    send3(w, IAC, WILL, opt).await
}
async fn send_wont(w: &mut OwnedWriteHalf, opt: u8) -> std::io::Result<()> {
    send3(w, IAC, WONT, opt).await
}
