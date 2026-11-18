#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{env, io};
use std::env::set_current_dir;
use std::io::{Write};
use std::process::Command;

fn main () {
    set_current_dir(env::home_dir().unwrap()).unwrap();
    let mut input = String::new();

    loop {
        print!("{} > ", env::current_dir().unwrap().display());
        io::stdout().flush().unwrap();

        input.clear();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                input = input.trim().to_string();

                let (command, args) = parse(input);
                input = String::new();

                match command.as_str() {
                    "cd" => {
                        if args.len() > 1 {
                            println!("Too many arguments.");
                        } else if args.len() == 0 {
                            set_current_dir(env::home_dir().unwrap()).unwrap();
                        }
                        else {
                            let mut newDir = env::current_dir().unwrap();
                            newDir.push(args[0].clone());
                            if newDir.exists() {
                                set_current_dir(newDir).unwrap();
                            }
                        }
                    },
                    "exit" => {
                        if args.len() > 1 {
                            println!("Too many arguments.");
                        }
                        else if args.len() == 0 {
                            std::process::exit(0);
                        }
                        else {
                            match args[0].parse::<i32>() {
                                Ok(_) => {
                                    std::process::exit(args[0].parse::<i32>().unwrap());
                                },
                                Err(_) => {
                                    println!("Invalid argument. Not a number.");
                                }
                            }
                        }
                    },
                    _ => {
                        let output = Command::new(command).args(args).spawn().unwrap().wait_with_output();
                        print!("{}", str::from_utf8(&*match output {
                            Ok(..) => {output.unwrap().stdout},
                            Err(..) => { output.unwrap().stderr }
                        }).unwrap());
                    }
                }
            }
            Err(_) => {
                print!("Error reading input");
            }
        }
    }
}

fn parse (input: String) -> (String, Vec<String>)  {
    let mut command = String::new();
    let mut arguments = Vec::new();
    let mut argument = String::new();
    let mut commandFound = false;
    let mut string = false;

    for character in input.chars() {
        if !commandFound {
            if character == ' ' {
                commandFound = true;
            }
            command.push(character);
        }
        else {
            if character == "'".chars().nth(0).unwrap() {
                string = !string;
            }
            if (character != ' ' || string) && character !=  "'".chars().nth(0).unwrap(){
                argument.push(character);
            }
            else if argument.len() > 0  && !string{
                arguments.push(argument);
                argument = String::new();
            }
        }
    }
    if argument.len() > 0 {
        arguments.push(argument);
    }
    let mut argumentPush = Vec::new();
    for argument in arguments.iter_mut() {
        if argument.len() > 1 {
            if argument.chars().nth(0).unwrap() == '-' && argument.chars().nth(1).unwrap() != '-' && argument.len() > 1 {
                for _ in 0..argument.len() - 2 {
                    argumentPush.push("-".to_string() + &argument.chars().nth(1).unwrap().to_string());
                    argument.remove(1);
                }
            }
        }
    }

    arguments.append(&mut argumentPush);

    (command.trim().to_string(), arguments)
}