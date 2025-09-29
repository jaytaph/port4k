use tokio::io::{AsyncBufRead, AsyncBufReadExt};

const IAC: u8 = 255;
const WILL: u8 = 251;
const WONT: u8 = 252;
const DO: u8 = 253;
const DONT: u8 = 254;
const SB: u8 = 250;
const SE: u8 = 240;
const ECHO: u8 = 1;
const SGA: u8 = 3;

pub(crate) async fn telnet_echo_off<W: tokio::io::AsyncWrite + Unpin>(w: &mut W) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    w.write_all(&[IAC, WILL, ECHO]).await?;
    w.write_all(&[IAC, DO, SGA]).await?;
    w.flush().await
}

pub(crate) async fn telnet_echo_on<W: tokio::io::AsyncWrite + Unpin>(w: &mut W) -> std::io::Result<()> {
    use tokio::io::AsyncWriteExt;
    w.write_all(&[IAC, WONT, ECHO]).await?;
    w.write_all(&[IAC, DONT, SGA]).await?;
    w.flush().await
}

fn strip_telnet(raw: &[u8]) -> Vec<u8> {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        let b = raw[i];
        if b == IAC {
            i += 1;
            if i >= raw.len() { break; }
            let cmd = raw[i];
            i += 1;
            match cmd {
                WILL | WONT | DO | DONT => { i += 1; } // consume option byte
                SB => {
                    // consume until IAC SE
                    while i < raw.len() {
                        if raw[i] == IAC && i + 1 < raw.len() && raw[i + 1] == SE {
                            i += 2;
                            break;
                        }
                        i += 1;
                    }
                }
                SE => {}           // end subnegotiation
                240..255 => {}    // other cmds (NOP/DM etc.) ignore
                255 => out.push(255), // IAC IAC (escaped 0xFF) -> literal 0xFF
                _ => {}
            }
            continue;
        }

        match b {
            b'\r' => {
                // telnet maps end of line as CR LF or CR NUL
                if i + 1 < raw.len() && (raw[i + 1] == b'\n' || raw[i + 1] == 0) {
                    i += 1;
                }
                out.push(b'\n');
            }
            0x08 | 0x7F => { // backspace/delete
                let _ = out.pop();
            }
            b if b < 0x20 && b != b'\n' && b != b'\t' => {
                // skip other control chars
            }
            _ => out.push(b),
        }
        i += 1;
    }
    out
}

/// Read one logical line from a Telnet stream (with negotiations), into a String.
pub(crate) async fn read_telnet_line<R: AsyncBufRead + Unpin>(reader: &mut R) -> std::io::Result<String> {
    let mut raw = Vec::new();
    // read until LF; CR-only lines will be normalized in strip_telnet
    let _n = reader.read_until(b'\n', &mut raw).await?;
    let clean = strip_telnet(&raw);
    Ok(String::from_utf8_lossy(&clean).to_string())
}