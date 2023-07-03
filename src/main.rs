use anyhow::{Context, Result};
use clap::{arg, command, Parser};
use itertools::Itertools;
use lazy_static::lazy_static;
use regex::Regex;
use rosc::{encoder, OscMessage, OscPacket, OscType};
use std::net::UdpSocket;
use std::process::{Command, Stdio};
use std::{thread, time};
use tracing::{error, info};

const LEVEL_KEY: &str = "  level: ";
const HANDLER_KEY: &str = "   handler: ";
const BATTERY_KEY: &str = "   battery: ";

lazy_static! {
    static ref REGEX_CONTROLLER_LEFT: Regex =
        Regex::new("handler: left[.\\s\\S]*?battery: ([0-9]*)").unwrap();
    static ref REGEX_CONTROLLER_RIGHT: Regex =
        Regex::new("handler: right[.\\s\\S]*?battery: ([0-9]*)").unwrap();
}

#[derive(Parser, Debug)]
#[command(author, version, about, long_about = None)]
struct Arguments {
    /// Receiver address
    #[arg(short, long, default_value_t = String::from("127.0.0.1:9000"))]
    receiver: String,

    /// Sender address
    #[arg(long, default_value_t = String::from("127.0.0.1:9003"))]
    sender: String,
}

fn main() {
    tracing_subscriber::fmt::init();

    let arguments = Arguments::parse();
    let socket = UdpSocket::bind(&arguments.sender).unwrap();
    let sleep_duration = time::Duration::from_secs(60);

    start_adb_server();

    loop {
        if let Ok(levels) = get_levels() {
            info!("{:?}", levels);

            let headset_message = OscPacket::Message(OscMessage {
                addr: String::from("/avatar/parameters/BatteryLevelHeadset"),
                args: vec![OscType::Float(levels.headset)],
            });
            let controller_left = OscPacket::Message(OscMessage {
                addr: String::from("/avatar/parameters/BatteryLevelControllerLeft"),
                args: vec![OscType::Float(levels.left_controller)],
            });
            let controller_right = OscPacket::Message(OscMessage {
                addr: String::from("/avatar/parameters/BatteryLevelControllerRight"),
                args: vec![OscType::Float(levels.right_controller)],
            });

            let headset_buffer = encoder::encode(&headset_message)
                .expect("Failed to encode headset battery level message");
            let controller_left_buffer = encoder::encode(&controller_left)
                .expect("Failed to encode left controller battery level message");
            let controller_right_buffer = encoder::encode(&controller_right)
                .expect("Failed to encode right controller battery level message");

            socket
                .send_to(&headset_buffer, &arguments.receiver)
                .expect("Failed to send headset battery level");
            socket
                .send_to(&controller_left_buffer, &arguments.receiver)
                .expect("Failed to send left controller battery level");
            socket
                .send_to(&controller_right_buffer, &arguments.receiver)
                .expect("Failed to send right controller battery level");
        } else {
            error!("Failed to retrieve battery levels");
        }

        thread::sleep(sleep_duration);
    }
}

fn start_adb_server() {
    info!("Starting adb server...");
    Command::new("adb")
        .arg("start-server")
        .stderr(Stdio::null())
        .stdout(Stdio::null())
        .status()
        .expect("Failed to start adb server");
    info!("Adb server started");
}

#[derive(Debug)]
struct BatteryLevels {
    pub headset: f32,
    pub left_controller: f32,
    pub right_controller: f32,
}

fn get_levels() -> Result<BatteryLevels> {
    let headset: u8 = get_battery_dump()
        .lines()
        .find(|line| line.starts_with(LEVEL_KEY))
        .context("Failed to find headset battery level")?
        .replace(LEVEL_KEY, "")
        .parse()
        .context("Failed to detach the important thing")?;

    let controllers: String = get_controller_service_dump()
        .lines()
        .filter(|line| line.starts_with(HANDLER_KEY) || line.starts_with(BATTERY_KEY))
        .intersperse("\n")
        .collect();
    let left_controller: u8 = REGEX_CONTROLLER_LEFT
        .captures_iter(&controllers)
        .next()
        .context("Failed to capture left controller battery level")?[1]
        .to_string()
        .parse()
        .context("Failed to parse left controller battery level")?;
    let right_controller: u8 = REGEX_CONTROLLER_RIGHT
        .captures_iter(&controllers)
        .next()
        .context("Failed to capture right controller battery level")?[1]
        .to_string()
        .parse()
        .context("Failed to parse right controller battery level")?;

    Ok(BatteryLevels {
        headset: headset as f32 / 100.0,
        left_controller: left_controller as f32 / 5.0,
        right_controller: right_controller as f32 / 5.0,
    })
}

fn get_battery_dump() -> String {
    String::from_utf8(
        Command::new("adb")
            .args(["shell", "dumpsys", "battery"])
            .stderr(Stdio::null())
            .output()
            .expect("Failed to get headset battery")
            .stdout,
    )
    .expect("Failed to convert headset battery output to a string")
}

fn get_controller_service_dump() -> String {
    String::from_utf8(
        Command::new("adb")
            .args(["shell", "dumpsys", "pxrcontrollerservice"])
            .stderr(Stdio::null())
            .output()
            .expect("Failed to get controller batteries")
            .stdout,
    )
    .expect("Failed to convert controller batteries output to a string")
}
