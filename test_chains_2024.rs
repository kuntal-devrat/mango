fn main() {
    let a = Some(1);
    let b = Some(2);
    if let Some(x) = a && let Some(y) = b {
        println!("{} {}", x, y);
    }
}
