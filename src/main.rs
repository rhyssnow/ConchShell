#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{env, io, thread};
use std::env::set_current_dir;
use std::fs::File;
use std::io::{stdout, BufRead, BufReader, Read, Stderr, Stdin, Stdout, Write};
use std::process::{Child, ChildStderr, ChildStdin, ChildStdout, Command, ExitStatus, Output, Stdio};
use std::thread::spawn;

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

                let mut piped = false;

                let processes = parse(input, &mut piped);
                input = String::new();

                let mut first = true;
                let mut last = false;

                let mut children = Vec::new();
                let processLen = processes.len();

                for (i, (command, args, mode)) in processes.into_iter().enumerate() {
                    let mut error = String::new();
                    if i == processLen - 1 {
                        last = true;
                    }
                    let mut child;
                    if first {
                        child = buildCommand(command, args, None, &mut error, first, last);
                    }
                    else {
                        let prevChild: &mut Child = &mut children.last_mut().unwrap();
                        let prevStdout = prevChild.stdout.take().unwrap();
                        child = buildCommand(command, args, Option::from(prevStdout), &mut error, first, last);
                    }
                    match child {
                        Some(child) => {
                            children.push(child);
                        },
                        None => {
                            eprintln!("{}", error);
                        }
                    }
                    first = false;
                }

                for mut child in children {
                    child.wait().unwrap();
                }
            }
            Err(_) => {
                print!("Error reading input");
            }
        }
    }
}

fn parse (input: String, piped: &mut bool) -> Vec<(String, Vec<String>, ([i32; 2], [String; 2]))>  {
    let mut command = String::new();
    let mut arguments = Vec::new();
    let mut argument = String::new();
    let mode = [0, 0];
    let file = [String::new(), String::new()];
    let mut commandFound = false;
    let mut string = false;
    let mut processes: Vec<(String, Vec<String>, ([i32; 2], [String; 2]))> = Vec::new();
    *piped = false;

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
            if character == "'".chars().nth(0).unwrap() || character == '"' {
                string = !string;
            }
            else if character == '|'{
                processes.push((command.trim().to_string(), arguments, (mode, file.clone())));
                commandFound = false;
                command = String::new();
                argument = String::new();
                arguments = Vec::new();
                *piped = true;
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
    processes.push((command.trim().to_string(), arguments, (mode, file)));


    for process in processes.iter_mut() {
        let mut argumentPush = Vec::new();
        let mut file = 0;
        let mut removeArg = Vec::new();
        for argument in process.1.iter_mut() {
            match argument.as_str() {
                ">" => {
                    file = 1;
                    process.2.0[0] = 1
                },
                ">>" => {
                    file = 1;
                    process.2.0[0] = 2
                },
                "<" => {
                    file = 2;
                    process.2.0[1] = 1
                },
                _ => {
                    if file != 0 {
                        if file == 1 {
                            process.2.1[0] = argument.clone();
                        }
                        else {
                            process.2.1[1] = argument.clone();
                        }
                        removeArg.push(argument.clone());
                    }
                    else if argument.len() > 1 {
                        if argument.chars().nth(0).unwrap() == '-' && argument.chars().nth(1).unwrap() != '-' && argument.len() > 1 {
                            for _ in 0..argument.len() - 2 {
                                argumentPush.push("-".to_string() + &argument.chars().nth(1).unwrap().to_string());
                                argument.remove(1);
                            }
                        }
                    }
                }
            }
        }
        for arg in removeArg {
            let pos = process.1.iter().position(|x| x == &arg).unwrap();
            process.1.remove(pos);
        }
        let mut removals = 0;
        const REDIRECTIONS: [&str; 3] = [">" , ">>" , "<" ];
        for argument in process.1.iter() {
            if REDIRECTIONS.contains(&argument.as_str()) {
                removals += 1;
            }
        }
        for _ in 0..removals {
            let pos = process.1.iter().position(|x| REDIRECTIONS.contains(&&**x)).unwrap();
            process.1.remove(pos);
        }
        process.1.append(&mut argumentPush);
    }

    processes
}

fn builtIns (command: String, args : Vec<String>, stdOutput: &mut String, error: &mut String) -> bool{
    match command.as_str() {
        "cd" => {
            if args.len() > 1 {
                *error = "Too many arguments".to_string();
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
                *error = "Too many arguments".to_string();
            } else if args.len() == 0 {
                std::process::exit(0);
            } else {
                match args[0].parse::<i32>() {
                    Ok(_) => {
                        std::process::exit(args[0].parse::<i32>().unwrap());
                    },
                    Err(_) => {
                        *error = "Invalid argumend: ".to_string()+ args[0].as_str();
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


fn buildCommand (command: String, args : Vec<String>, prevStdin: Option<ChildStdout>, error: &mut String, first: bool, last: bool) -> Option<Child> {
    let mut commandResult = Command::new(command);
    commandResult.args(args);
    if first {
        commandResult.stdin(Stdio::inherit());
    } else {
        commandResult.stdin(Stdio::from(prevStdin.unwrap()));
    }

    if last {
        commandResult.stdout(Stdio::inherit()).stderr(Stdio::inherit());
    }
    else {
        commandResult.stdout(Stdio::piped()).stderr(Stdio::piped());
    }
    let result = commandResult.spawn();

    let command;
    match result {
        Ok(child) => command = child,
        Err(..) => {*error = result.unwrap_err().to_string() + "\n";
            return None},
    };
    Some(command)
}