use std::{env, io};
use std::io::{Read, Write};

fn main (){
    let mut env = env::home_dir().unwrap();
    let mut input = String::new();

    loop {
        print!("{} > ", env.to_str().unwrap());
        io::stdout().flush().unwrap();

        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                print!("Your input was: {}", input);
            }
            Err(_) => {
                print!("Error reading input");
            }
        }
    }
}