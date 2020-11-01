use futures_util::{future, pin_mut, StreamExt};
use slog::*;
use std::io;
use tokio_tungstenite::connect_async;
use tokio_tungstenite::tungstenite::Message;

pub async fn connect(addr: String, log: Logger) -> io::Result<()> {
    use tokio::io::AsyncWriteExt as _;

    let log = log.new(o!("connect" => addr.clone()));

    info!(log, "Connecting");
    let (ws_stream, _) = connect_async(addr).await.unwrap();
    let (write, read) = ws_stream.split();

    // stdin -> ws
    let (stdin_tx, stdin_rx) = futures_channel::mpsc::unbounded();
    tokio::spawn(read_stdin(stdin_tx, log.clone()));
    let stdin_to_ws = stdin_rx.map(Ok).forward(write);

    // ws -> stdout
    let ws_to_stdout = read.for_each(|msg| async {
        let msg = msg.unwrap();
        debug!(log, "<<< {}", msg);
        let data = msg.into_data();
        tokio::io::stdout().write_all(&data).await.unwrap();
    });

    info!(log, "Connected and ready");
    pin_mut!(stdin_to_ws, ws_to_stdout);
    future::select(stdin_to_ws, ws_to_stdout).await;

    Ok(())
}

async fn read_stdin(tx: futures_channel::mpsc::UnboundedSender<Message>, log: Logger) {
    use tokio::io::AsyncReadExt as _;

    let mut stdin = tokio::io::stdin();
    loop {
        let mut buf = vec![0; 1024];
        let n = match stdin.read(&mut buf).await {
            Err(_) | Ok(0) => break,
            Ok(n) => n,
        };
        buf.truncate(n);
        debug!(log, ">>> {}", String::from_utf8_lossy(&buf));
        tx.unbounded_send(Message::binary(buf)).unwrap();
    }
}

pub async fn accept(addr: String, log: Logger) -> io::Result<()> {
    use futures_util::SinkExt as _;
    use std::process::Stdio;
    use tokio::io::AsyncBufReadExt as _;
    use tokio::io::AsyncWriteExt as _;
    use tokio::io::BufReader;
    use tokio::process::Command;
    use tokio::sync::Mutex;

    let log = log.new(o!("accept" => addr.clone()));

    info!(log, "Connecting");
    let (ws_stream, _) = connect_async(addr).await.unwrap();
    let (write, read) = ws_stream.split();

    let mut pwsh_handle = Command::new("pwsh")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .arg("-sshs")
        .arg("-NoLogo")
        .arg("-NoProfile")
        .spawn()
        .unwrap();

    // stdout -> ws
    let stdout = pwsh_handle.stdout.take().unwrap();
    let write = Mutex::new(write);
    let stdout_to_ws = BufReader::new(stdout).lines().for_each(|line| async {
        let line = line.unwrap();
        debug!(log, ">>> {}", line);
        let bytes = line.into_bytes();
        let msg = Message::Binary(bytes);
        write.lock().await.send(msg).await.unwrap();
    });

    // ws -> stdin
    let stdin = Mutex::new(pwsh_handle.stdin.take().unwrap());
    let ws_to_stdin = read.for_each(|msg| async {
        let msg = msg.unwrap();
        debug!(log, "<<< {}", msg);
        let data = msg.into_data();
        stdin.lock().await.write_all(&data).await.unwrap();
    });

    info!(log, "Connected and ready");
    pin_mut!(stdout_to_ws, ws_to_stdin);
    future::select(stdout_to_ws, ws_to_stdin).await;

    Ok(())
}
