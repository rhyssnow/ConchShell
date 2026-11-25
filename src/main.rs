#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{env, thread};
use std::env::set_current_dir;
use std::fs::{ File, OpenOptions, exists};
use std::process::{Child, Command, Stdio};
use rustyline::{Cmd, DefaultEditor, KeyEvent, KeyCode, Modifiers};
use rustyline::error::ReadlineError;
use std::os::unix::process::CommandExt;
use hostname::get;
use colored::Colorize;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

fn main (){
    let mut rl = DefaultEditor::new().unwrap();
    let user = env::var("USER").unwrap();
    let shellPgid = unsafe { libc::getpgrp()};
    unsafe {
        libc::signal(libc::SIGTTOU, libc::SIG_IGN);
    }
    if !exists(env::home_dir().unwrap().to_str().unwrap().to_string() + "/.conch_history").unwrap() {
        File::create(env::home_dir().unwrap().to_str().unwrap().to_string() + "/.conch_history").unwrap();
    }

    rl.bind_sequence(KeyEvent(KeyCode::Up, Modifiers::NONE), Cmd::PreviousHistory);
    rl.bind_sequence(KeyEvent(KeyCode::Down, Modifiers::NONE), Cmd::NextHistory);

    let historyPath = env::home_dir().unwrap().join(".conch_history");

    rl.load_history(&historyPath).unwrap();

    let term = Arc::new(AtomicBool::new(false));
    for sig in [signal_hook::consts::SIGHUP, signal_hook::consts::SIGTERM, signal_hook::consts::SIGQUIT] {
        signal_hook::flag::register(sig, Arc::clone(&term)).unwrap();
    }

    while !term.load(Ordering::Relaxed) {
        println!("{}{}{} {} ",
                 user.clone().purple().bold(),
            "@".bold(),
                 get().unwrap().to_str().unwrap().red().bold(),
            match env::current_dir().unwrap().file_name() {
                Some(dir) => {
                    dir.to_str().unwrap().blue()
                }
                None => {
                    "/".blue()
                }
            }
        );

        let input = rl.readline("   > ");
        match input  {
            Ok(mut input) => {
                input = input.trim().to_string();
                rl.add_history_entry(&input).unwrap();
                run(input);

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
    rl.save_history(&historyPath).unwrap();
}

fn run (input: String) {

    let processes = parse(input);

    let mut prevPiped = false;
    let mut execute = true;

    let mut children = Vec::new();

    let mut pgid: Option<i32> = None;

    for (command, args, mut mode, outputDirection, group) in processes.0.into_iter() {
        if execute {
            let mut error = String::new();
            if !builtIns(command.clone(), args.clone(), &mut error) {
                let mut error = String::new();

                let mut prevStdout;
                if !prevPiped {
                    prevStdout = Option::from(None);
                } else {
                    let prevChild: &mut Child = &mut children.last_mut().unwrap();
                    prevStdout = Option::from(Stdio::from(prevChild.stdout.take().unwrap()));
                }
                let mut stdout: Option<Stdio> = Option::from(None);
                let mut stderr: Option<Stdio> = Option::from(None);
                match mode.0[1] {
                    1 => {
                        match File::open(mode.1[1].clone()) {
                            Ok(file) => {
                                prevStdout = Option::from(Stdio::from(file));
                                prevPiped = true;
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
                    1 => {
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
                    1 => {
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
                let child = buildCommand(command, args, prevStdout, stdout, stderr, &mut error, outputDirection.clone(), prevPiped, pgid);
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
                match outputDirection.as_str() {
                    "pipe" => {
                        prevPiped = true;
                        execute = true;
                    },
                    "then" => {
                        prevPiped = false;

                        children.last_mut().unwrap().wait().unwrap();
                        children.pop();
                        pgid = None;

                        execute = true;
                    },
                    "or" => {
                        prevPiped = false;

                        let exitCode = children.last_mut().unwrap().wait().unwrap();
                        match exitCode.code().unwrap() {
                            0 => {
                                execute = false;
                            }
                            _ => {
                                execute = true;
                                pgid = None;
                            }
                        }

                        children.pop();
                    },
                    "and" => {
                        prevPiped = false;

                        let exitCode = children.last_mut().unwrap().wait().unwrap();
                        match exitCode.code().unwrap() {
                            0 => {
                                execute = true;
                                pgid = None;
                            }
                            _ => {
                                execute = false;
                            }
                        }

                        children.pop();
                    },
                    "background" => {
                        prevPiped = false;

                        execute = true;

                        let mut child = children.pop().unwrap();
                        children.pop();
                        thread::spawn(move || {
                           child.wait().unwrap();
                        });
                    }
                    _ => {
                        prevPiped = false;
                        execute = true;
                    }
                }
            }
            else {
                eprint!("{}", error);
            }
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
}

fn parse (input: String) -> (Vec<(String, Vec<String>, ([i32; 4], [String; 3]), String, i32)>, Vec<String>, Vec<(i32, ([i32; 4], [String; 3]))>)  {
    let mut arguments: Vec<(String, i32)> = Vec::new();
    let mut argument = String::new();
    let mut commandFound;
    let mut string = false;
    let mut subShell = false;
    let mut processes: Vec<(String, Vec<String>, ([i32; 4], [String; 3]), String, i32)> = Vec::new();
    let mut subShells: Vec<String> = Vec::new();
    let mut subshell= String::new();
    let mut currentGroup = 0;
    let mut maxGroup = 0;

    for character in input.chars() {
        if subShell {
            if character == ')' {
                subShell = false;
                subShells.push(subshell);
                arguments.push(("subshell".to_string(), 0));
                subshell = String::new();
                continue
            }
            subshell.push(character);
        }
        else {
            if character == '{' {
                currentGroup = maxGroup + 1;
                maxGroup += 1;
            }
            else if character == '}'  {
                arguments.push((argument, currentGroup,));
                argument = String::new();
                currentGroup = 0;
            }
            else if character == '(' {
                subShell = true;
            } else if character == "'".chars().nth(0).unwrap() || character == '"' {
                string = !string;
            } else if character != ' ' || string {
                argument.push(character);
            } else if argument.len() > 0 && !string {
                arguments.push((argument, currentGroup));
                argument = String::new();
            }
        }
    }
    if argument.len() > 0 {
        arguments.push((argument, currentGroup));
    }

    commandFound = false;
    let mut newProcess: (String, Vec<String>, ([i32; 4], [String; 3])) = (String::new(), Vec::new(), ([0, 0, 0, 0], [String::new(), String::new(), String::new()]));

    let mut prevGroup = 0;
    let mut inBetween = false;
    let mut groupThings = Vec::new();

    for argument in arguments.iter_mut() {
        if !commandFound {
            newProcess.0 = argument.clone().0;
            currentGroup = argument.1;
            commandFound = true;
        }
        else {
            commandFound = false;
            match argument.0.as_str() {
                "|" => {
                    processes.push((newProcess.0, newProcess.1, newProcess.2, "pipe".to_string(), currentGroup));
                    newProcess = (String::new(), Vec::new(), ([0, 0, 0, 0], [String::new(), String::new(), String::new()]));
                    inBetween = false;
                }
                "||" => {
                    processes.push((newProcess.0, newProcess.1, newProcess.2, "or".to_string(), currentGroup));
                    newProcess = (String::new(), Vec::new(), ([0, 0, 0, 0], [String::new(), String::new(), String::new()]));
                    inBetween = false;
                }
                "&&" => {
                    processes.push((newProcess.0, newProcess.1, newProcess.2, "and".to_string(), currentGroup));
                    newProcess = (String::new(), Vec::new(), ([0, 0, 0, 0], [String::new(), String::new(), String::new()]));
                    inBetween = false;
                }
                ";" => {
                    processes.push((newProcess.0, newProcess.1, newProcess.2, "then".to_string(), currentGroup));
                    newProcess = (String::new(), Vec::new(), ([0, 0, 0, 0], [String::new(), String::new(), String::new()]));
                    inBetween = false;
                }
                "&" => {
                    processes.push((newProcess.0, newProcess.1, newProcess.2, "background".to_string(), currentGroup));
                    newProcess = (String::new(), Vec::new(), ([0, 0, 0, 0], [String::new(), String::new(), String::new()]));
                    inBetween = false;
                }
                _ => {
                    if prevGroup != argument.1 && !commandFound && prevGroup != 0 {
                        inBetween = true;

                        //put command sorting logic here
                    }
                    else if !inBetween {
                        commandFound = true;
                        newProcess.1.push(argument.clone().0);
                    }
                }
            }
        }
        prevGroup = argument.1;
    }

    processes.push((newProcess.0, newProcess.1, newProcess.2, "normal".to_string(), currentGroup));

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
                               else if argument.chars().nth(0).unwrap() == '$' {
                                   argument.remove(0);
                                   *argument = match env::var(argument.clone()) {
                                       Ok(value) => value,
                                       Err(_) => {
                                           println!("Error reading argument: {}", argument);
                                           removeArg.push(argument.clone());
                                           argument.clone()
                                       }
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

    (processes, subShells, groupThings)
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


fn buildCommand (command: String, args : Vec<String>, prevStdin: Option<Stdio>, fileOut: Option<Stdio>, fileErr: Option<Stdio>, error: &mut String, mode: String, prevPipe: bool, pgid: Option<i32>) -> Option<Child> {
    let mut commandResult = Command::new(command);
    commandResult.process_group(pgid.unwrap_or(0));
    commandResult.args(args).stderr(Stdio::inherit()).stdout(Stdio::inherit()).stdin(Stdio::inherit());

    if mode.as_str() == "pipe" {
        commandResult.stdout(Stdio::piped());
    }
    if prevPipe {
        commandResult.stdin(prevStdin.unwrap());
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
        Err(..) => {
            *error = result.unwrap_err().to_string() + "\n";
            return None},
    };
    Some(command)
}