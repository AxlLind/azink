async fn test() -> i32 {
    1
}

fn main() {
    assync::block_on(async {
        println!("Hello, world!");
        let x = test().await;
        println!("x = {}", x);
        assync::spawn(async {
            println!("Inside spawned task");
        });
        let y = test().await;
        println!("y = {}", y);
    });
}
