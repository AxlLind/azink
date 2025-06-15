async fn test() -> i32 {
    1
}

fn main() {
    azink::block_on(async {
        println!("Hello, world!");
        let x = test().await;
        println!("x = {}", x);
        azink::spawn(async {
            println!("Inside spawned task");
        });
        let y = test().await;
        println!("y = {}", y);
    });
}
