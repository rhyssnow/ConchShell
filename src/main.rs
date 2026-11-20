#![allow(non_camel_case_types)]
#![allow(non_snake_case)]

use std::{env, io};
use std::env::set_current_dir;
use std::io::{BufRead, BufReader, Write};
use std::process::{Command, Output, Stdio};

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

                if !piped {
                    let mut output = String::new();
                    let mut error = String::new();
                    let (command, args) = processes[0].clone();
                    if !builtIns(command.clone(), args.clone(), &mut output, &mut error) {
                        executeCommand(command, args, &mut output, &mut error);
                    }
                    print!("{}", output);
                    print!("{}", error);
                }
                else {
                    let mut output = String::new();
                    let mut error = String::new();

                    for (command, args) in processes.iter() {
                        if !builtIns(command.clone(), args.clone(), &mut output, &mut error) {
                            executeCommandPiped(command.clone(), args.clone(), &mut output, &mut error);
                        }
                    }
                    print!("{}", output);
                    print!("{}", error);
                }
            }
            Err(_) => {
                print!("Error reading input");
            }
        }
    }
}

fn parse (input: String, piped: &mut bool) -> Vec<(String, Vec<String>)>  {
    let mut command = String::new();
    let mut arguments = Vec::new();
    let mut argument = String::new();
    let mut commandFound = false;
    let mut string = false;
    let mut process: Vec<(String, Vec<String>)> = Vec::new();
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
            if character == "'".chars().nth(0).unwrap() {
                string = !string;
            }
            else if character == '|'{
                process.push((command.trim().to_string(), arguments));
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

fn executeCommand (command: String, args : Vec<String>, stdOutput: &mut String, error: &mut String) {
    let spawn = Command::new(command).args(args).spawn();
    let output: Output;
    match spawn {
        Ok(..) => {
            output = spawn.unwrap().wait_with_output().unwrap();
            *stdOutput = String::from_utf8(output.stdout).unwrap();
        },
        Err(..) => {
            *error = spawn.unwrap_err().to_string();
        }
    }
}

fn executeCommandPiped (command: String, args : Vec<String>, prevStdin: &mut String, error: &mut String) {
    let command = Command::new(command.clone()).args(args.clone()).stdout(Stdio::piped()).stdin(Stdio::piped()).stderr(Stdio::piped()).spawn();
    let mut command = match command {
        Ok(command) => command,
        Err(..) => {*error = command.unwrap_err().to_string() + "\n";
        return;},
    };

    let stdout = command.stdout.take().unwrap();
    let stderr = command.stderr.take().unwrap();

    let outThread = std::thread::spawn(move || {
        let reader = BufReader::new(stdout);
        let mut output: String = String::new();

        for line in reader.lines() {
            match line {
                Ok(line) => {
                    output.push_str(&line);
                    output.push('\n');
                }
                Err(..) => {
                    eprintln!("Error reading stdout");
                }
            }
        }
        return output;
    });

    let errThread = std::thread::spawn(move || {
        let reader = BufReader::new(stderr);
        let mut output: String = String::new();

        for line in reader.lines() {
            match line {
                Ok(line) => {
                    output.push_str(&line);
                    output.push('\n');
                }
                Err(..) => {
                    eprintln!("Error reading stdout");
                }
            }
        }
        return output;
    });

    let mut stdin = command.stdin.take().unwrap();

    if !prevStdin.is_empty() {
        stdin.write_all(prevStdin.as_bytes()).unwrap();
        stdin.flush().unwrap();
        drop(stdin);
    }

    let status = command.wait().unwrap();
    if !status.success() {
        eprintln!("{}", status);
    }


    *prevStdin = outThread.join().unwrap();
    *error = errThread.join().unwrap();

}