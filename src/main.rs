fn main() {
    azink::block_on(async {
        let listener = azink::net::TcpListener::bind("127.0.0.1:8080").unwrap();
        loop {
            let (mut stream, addr) = listener.accept().await.unwrap();
            azink::spawn(async move {
                let mut buf = [0; 1024];
                let bytes_read = stream.read(&mut buf).await.unwrap();
                println!("Read {} bytes from {}: {:?}", bytes_read, addr, String::from_utf8_lossy(&buf[..bytes_read]));

                let response = b"HTTP/1.1 200 OK\r\nContent-Length: 13\r\n\r\nHello, world!";
                let mut bytes_written = 0;
                while bytes_written < response.len() {
                    bytes_written += stream.write(&response[bytes_written..]).await.unwrap();
                }
                println!("Sent response to {}", addr);
            });
        }
    });
}
