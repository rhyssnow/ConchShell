#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{env, io};
use std::env::set_current_dir;
use std::fs::{File, OpenOptions};
use std::io::{Write};
use std::process::{Child, Command, Stdio};
use rustyline::{ DefaultEditor,};
use rustyline::error::ReadlineError;
use std::os::unix::process::CommandExt;

fn main () {
    let mut rl = DefaultEditor::new().unwrap();
    let shellPgid = unsafe { libc::getpgrp()};
    unsafe {
        libc::signal(libc::SIGTTOU, libc::SIG_IGN);
    }


    loop {
        print!("{} > ", env::current_dir().unwrap().display());
        io::stdout().flush().unwrap();

        let input = rl.readline(&(env::current_dir().unwrap().display().to_string() + " > "));
        match input  {
            Ok(mut input) => {
                input = input.trim().to_string();

                let mut piped = false;

                let processes = parse(input, &mut piped);

                let mut first = true;
                let mut last = false;


                let mut children = Vec::new();
                let processLen = processes.len();

                let mut pgid: Option<i32> = None;

                for (i, (command, args, mut mode)) in processes.into_iter().enumerate() {
                    let mut error = String::new();
                    if !builtIns(command.clone(), args.clone(), &mut error) {
                        let mut error = String::new();
                        if i == processLen - 1 {
                            last = true;
                        }
                        let prevStdout;
                        if first {
                            prevStdout = Option::from(None);
                        }
                        else {
                            let prevChild: &mut Child = &mut children.last_mut().unwrap();
                            prevStdout = Option::from(Stdio::from(prevChild.stdout.take().unwrap()));
                        }
                        let mut stdout: Option<Stdio> = Option::from(None);
                        let mut stderr: Option<Stdio> = Option::from(None);
                        match mode.0[1] {
                            1 => {
                                match File::open(mode.1[1].clone()) {
                                    Ok(file) => {
                                        stdout = Option::from(Stdio::from(file));
                                        first = false;
                                    },
                                    Err(_) => {
                                        eprintln!("Error opening file: {}", mode.1[1]);
                                        continue;
                                    }
                                }
                            }
                            _ => {}
                        }
                        match mode.0[3] {
                            1 => {
                                mode.1[2] = mode.1[0].clone();
                                mode.0[2] = mode.0[0].clone();
                            },
                            2 => {
                                mode.1[0] = mode.1[2].clone();
                                mode.0[0] = mode.0[2].clone();
                            },
                            _ => {}
                        }
                        match mode.0[2] {
                            1  => {
                                let file = OpenOptions::new().append(true).create(true).open(mode.1[2].clone()).unwrap();
                                file.set_len(0).unwrap();
                                stderr = Option::from(Stdio::from(file));
                            }
                            2 => {
                                let file = OpenOptions::new().append(true).create(true).open(mode.1[2].clone()).unwrap();
                                stderr = Option::from(Stdio::from(file));
                            }
                            _ => {}
                        }
                        match mode.0[0] {
                            1  => {
                                let file = OpenOptions::new().append(true).create(true).open(mode.1[0].clone()).unwrap();
                                file.set_len(0).unwrap();
                                stdout = Option::from(Stdio::from(file));
                            }
                            2 => {
                                let file = OpenOptions::new().append(true).create(true).open(mode.1[0].clone()).unwrap();
                                stdout = Option::from(Stdio::from(file));
                            }
                            _ => {}
                        }
                        let child = buildCommand(command, args, prevStdout, stdout, stderr, &mut error, first, last, pgid);
                        match child {
                            Some(child) => {
                                if pgid.is_none() {
                                    pgid = Some(child.id() as i32);
                                }
                                children.push(child);
                            },
                            None => {
                                eprintln!("{}", error);
                            }
                        }
                        first = false;
                    }
                    else {
                        eprint!("{}", error);
                    }
                }

                if let Some(pgid) = pgid {
                    unsafe {
                        libc::tcsetpgrp(libc::STDIN_FILENO, pgid);
                    }
                }

                for mut child in children {
                    child.wait().unwrap();
                }

                unsafe {
                    libc::tcsetpgrp(libc::STDIN_FILENO, shellPgid);
                }
            }
            Err(ReadlineError::Interrupted) => {
                continue
            }
            Err(ReadlineError::Eof) => {
                break
            }
            Err(_) => {
                print!("Error reading input");
            }
        }
    }
}

fn parse (input: String, piped: &mut bool) -> Vec<(String, Vec<String>, ([i32; 4], [String; 3]))>  {
    let mut command = String::new();
    let mut arguments = Vec::new();
    let mut argument = String::new();
    let mode = [0, 0, 0, 0];
    let file = [String::new(), String::new(), String::new()];
    let mut commandFound = false;
    let mut string = false;
    let mut processes: Vec<(String, Vec<String>, ([i32; 4], [String; 3]))> = Vec::new();
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
                "2>" => {
                    file = 3;
                    process.2.0[2] = 1
                },
                "2>>" => {
                    file = 3;
                    process.2.0[2] = 2
                },
                "<" => {
                    file = 2;
                    process.2.0[1] = 1
                },
                "2>&1" => {
                    process.2.0[3] = 1
                },
                "1>&2" => {
                    process.2.0[3] = 2
                },
                _ => {
                    match file {
                        1 => {
                            process.2.1[0] = argument.clone();
                            removeArg.push(argument.clone());
                        },
                        2 => {
                            process.2.1[1] = argument.clone();
                            removeArg.push(argument.clone());
                        },
                        3 => {
                            process.2.1[2] = argument.clone();
                            removeArg.push(argument.clone());
                        },
                        _ => {
                           if argument.len() > 1 {
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
            }
        }
        for arg in removeArg {
            let pos = process.1.iter().position(|x| x == &arg).unwrap();
            process.1.remove(pos);
        }
        let mut removals = 0;
        const REDIRECTIONS: [&str; 7] = [">" , ">>" , "<", "2>", "2>>", "2>&1", "1>&2"];
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

fn builtIns (command: String, args : Vec<String>, error: &mut String) -> bool{
    match command.as_str() {
        "cd" => {
            if args.len() > 1 {
                *error = "Too many arguments\n".to_string();
            } else if args.len() == 0 {
                set_current_dir(env::home_dir().unwrap()).unwrap();
            } else {
                let mut newDir = env::current_dir().unwrap();
                newDir.push(args[0].clone());
                if newDir.exists() {
                    set_current_dir(newDir).unwrap();
                }
                else {
                    *error = "No such directory\n".to_string();
                }
            }
            true
        },
        "exit" => {
            if args.len() > 1 {
                *error = "Too many arguments\n".to_string();
            } else if args.len() == 0 {
                std::process::exit(0);
            } else {
                match args[0].parse::<i32>() {
                    Ok(_) => {
                        std::process::exit(args[0].parse::<i32>().unwrap());
                    },
                    Err(_) => {
                        *error = "Invalid argument: ".to_string()+ args[0].as_str()+"\n";
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


fn buildCommand (command: String, args : Vec<String>, prevStdin: Option<Stdio>, fileOut: Option<Stdio>, fileErr: Option<Stdio>, error: &mut String, first: bool, last: bool, pgid: Option<i32>) -> Option<Child> {
    let mut commandResult = Command::new(command);
    commandResult.process_group(pgid.unwrap_or(0));

    commandResult.args(args).stderr(Stdio::inherit());
    if first {
        commandResult.stdin(Stdio::inherit());
    } else {
        commandResult.stdin(prevStdin.unwrap());
    }

    if last {
        commandResult.stdout(Stdio::inherit());
    }
    else {
        commandResult.stdout(Stdio::piped());
    }
    if fileOut.is_some() {
        commandResult.stdout(fileOut.unwrap());
    }
    if fileErr.is_some() {
        commandResult.stderr(fileErr.unwrap());
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