#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{env, io};
use std::env::set_current_dir;
use std::io::{ Write};
use std::process::{Command, Stdio};

fn main () {
    set_current_dir(env::home_dir().unwrap()).unwrap();
    let mut input = String::new();
    let mut pipe;

    loop {
        print!("{} > ", env::current_dir().unwrap().display());
        io::stdout().flush().unwrap();

        input.clear();
        match io::stdin().read_line(&mut input) {
            Ok(_) => {
                input = input.trim().to_string();
                pipe = false;

                let processes = parse(input, &mut pipe);
                input = String::new();

                if !pipe {
                    let command = processes[0].0.clone();
                    let args = processes[0].1.clone();
                    if !builtIns(command.clone(), args.clone()) {
                        print!("{}", executeCommand(command, args,"".to_string()));
                    }
                } else {
                    let mut output = String::new();
                    for (command, args) in processes {
                        output = executeCommand(command, args, output);
                    }
                    print!("{}", output);
                }
            }
            Err(_) => {
                print!("Error reading input");
            }
        }
    }
}

fn parse (input: String, pipe: &mut bool) -> Vec<(String, Vec<String>)>  {
    let mut command = String::new();
    let mut arguments = Vec::new();
    let mut argument = String::new();
    let mut commandFound = false;
    let mut string = false;
    let mut process: Vec<(String, Vec<String>)> = Vec::new();

    for character in input.chars() {
        if !commandFound {
            if character == ' ' && !command.is_empty() {
                commandFound = true;
            }
            else if character != ' ' {
                command.push(character);
            }
        }
        else {
            if character == "'".chars().nth(0).unwrap() {
                string = !string;
            }
            else if character == '|'{
                process.push((command.trim().to_string(), arguments));
                commandFound = false;
                command = String::new();
                argument = String::new();
                arguments = Vec::new();
                *pipe = true;
            }
            else if character != ' ' || string {
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

    process.push((command.trim().to_string(), arguments));
    process
}

fn builtIns (command: String, args : Vec<String>) -> bool {
    match command.as_str() {
        "cd" => {
            if args.len() > 1 {
                println!("Too many arguments");
            } else if args.len() == 0 {
                set_current_dir(env::home_dir().unwrap()).unwrap();
            } else {
                let mut newDir = env::current_dir().unwrap();
                newDir.push(args[0].clone());
                if newDir.exists() {
                    set_current_dir(newDir).unwrap();
                }
            }
            true
        },
        "exit" => {
            if args.len() > 1 {
                println!("Too many arguments");
            } else if args.len() == 0 {
                std::process::exit(0);
            } else {
                match args[0].parse::<i32>() {
                    Ok(_) => {
                        std::process::exit(args[0].parse::<i32>().unwrap());
                    },
                    Err(_) => {
                        println!("Invalid argument: {}", args[0]);
                    }
                }
            }
            true
        },
        _ => {
            false
        }
    }
}

fn executeCommand (command: String, args : Vec<String>, prevStdin: String) -> String {
    let command = Command::new(command.clone()).args(args.clone()).stdout(Stdio::piped()).stdin(Stdio::piped()).spawn();
    let mut command = match command {
        Ok(..) => command.unwrap(),
        Err(..) => return command.unwrap_err().to_string() + "\n",
    };
    if !prevStdin.is_empty() {
        let mut stdin = command.stdin.take().unwrap();
        stdin.write_all(prevStdin.as_bytes()).unwrap();
    }

    String::from_utf8(command.wait_with_output().unwrap().stdout).unwrap()
}