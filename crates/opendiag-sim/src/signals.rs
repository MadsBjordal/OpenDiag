//! Live values for the simulated vehicles: a repeating 60 s drive cycle.

use crate::profiles::Engine;

#[derive(Debug, Clone, Copy)]
pub struct Signals {
    pub rpm: f64,
    pub speed: f64,
    pub coolant: f64,
    pub iat: f64,
    pub throttle: f64,
    pub load: f64,
    pub map: f64,
    pub maf: f64,
    pub stft: f64,
    pub ltft: f64,
    pub o2: f64,
    pub voltage: f64,
    pub timing: f64,
    pub rail_kpa: f64,
    pub fuel_rate: f64,
}

pub fn at(t: f64, engine: Engine, running: bool, low_voltage: bool) -> Signals {
    let (idle, top) = match engine {
        Engine::Petrol => (780.0, 3000.0),
        Engine::Diesel => (800.0, 2600.0),
        Engine::MotorcyclePetrol => (1250.0, 7000.0),
    };
    let phase = t % 60.0;
    // 0–15 idle, 15–25 accelerate, 25–45 cruise, 45–60 slow down.
    let (drive, accel) = if phase < 15.0 {
        (0.0, 0.0)
    } else if phase < 25.0 {
        ((phase - 15.0) / 10.0, 1.0)
    } else if phase < 45.0 {
        (1.0, 0.3)
    } else {
        (1.0 - (phase - 45.0) / 15.0, 0.0)
    };
    let wobble = (t * 1.7).sin();
    let mut rpm = if running { idle + (top - idle) * drive * (0.6 + 0.4 * accel) + 25.0 * wobble } else { 0.0 };
    if rpm < 0.0 {
        rpm = 0.0;
    }
    let top_speed = if engine == Engine::MotorcyclePetrol { 90.0 } else { 70.0 };
    let speed = if running { (top_speed * drive).round() } else { 0.0 };
    let coolant = if running { (20.0 + t * 0.6).min(89.0) + wobble } else { 20.0 };
    let throttle = if running { 3.0 + 40.0 * accel * drive + 12.0 * drive } else { 0.0 };
    let load = if running { 18.0 + 50.0 * accel * drive + 10.0 * drive } else { 0.0 };
    let map = match (running, engine) {
        (false, _) => 101.0,
        (true, Engine::Diesel) => 101.0 + 110.0 * drive * (0.5 + 0.5 * accel),
        (true, _) => 32.0 + 55.0 * accel * drive + 10.0 * drive,
    };
    let maf = if running { rpm / 1000.0 * load / 6.0 } else { 0.0 };
    let stft = if running { 3.0 * (t * 2.3).sin() } else { 0.0 };
    let ltft = if running && engine == Engine::Petrol { 13.3 } else { 0.0 };
    let o2 = if running { 0.45 + 0.4 * (t * 3.1).sin() } else { 0.0 };
    let voltage = match (low_voltage, running) {
        (true, _) => 10.6,
        (false, true) => 14.1 + 0.05 * wobble,
        (false, false) => 12.5,
    };
    Signals {
        rpm,
        speed,
        coolant,
        iat: 24.0 + 3.0 * drive,
        throttle,
        load,
        map,
        maf,
        stft,
        ltft,
        o2,
        voltage,
        timing: if running { 10.0 + 15.0 * drive } else { 0.0 },
        rail_kpa: if running { 30000.0 + 90000.0 * drive } else { 0.0 },
        fuel_rate: if running { 0.8 + 6.0 * drive * (0.5 + accel) } else { 0.0 },
    }
}

fn clamp_u8(v: f64) -> u8 {
    v.round().clamp(0.0, 255.0) as u8
}

fn u16_bytes(v: f64) -> Vec<u8> {
    let n = v.round().clamp(0.0, 65535.0) as u16;
    vec![(n >> 8) as u8, (n & 0xFF) as u8]
}

/// Encodes a PID's value with the inverse of the SAE J1979 formula.
pub fn encode(pid: u8, s: &Signals, year: u16, fuel: Engine) -> Option<Vec<u8>> {
    Some(match pid {
        0x03 => vec![if s.rpm > 0.0 { 2 } else { 0 }, 0],
        0x04 => vec![clamp_u8(s.load * 255.0 / 100.0)],
        0x05 => vec![clamp_u8(s.coolant + 40.0)],
        0x06 => vec![clamp_u8(s.stft * 128.0 / 100.0 + 128.0)],
        0x07 => vec![clamp_u8(s.ltft * 128.0 / 100.0 + 128.0)],
        0x0B => vec![clamp_u8(s.map)],
        0x0C => u16_bytes(s.rpm * 4.0),
        0x0D => vec![clamp_u8(s.speed)],
        0x0E => vec![clamp_u8((s.timing + 64.0) * 2.0)],
        0x0F => vec![clamp_u8(s.iat + 40.0)],
        0x10 => u16_bytes(s.maf * 100.0),
        0x11 => vec![clamp_u8(s.throttle * 255.0 / 100.0)],
        0x13 => vec![0x03],
        0x14 | 0x15 => vec![clamp_u8(s.o2 * 200.0), 0x80],
        0x1C => vec![if year >= 2001 { 6 } else { 1 }],
        0x1F => u16_bytes(if s.rpm > 0.0 { 420.0 } else { 0.0 }),
        0x21 => u16_bytes(37.0),
        0x23 => u16_bytes(s.rail_kpa / 10.0),
        0x2F => vec![clamp_u8(62.0 * 255.0 / 100.0)],
        0x31 => u16_bytes(1840.0),
        0x33 => vec![101],
        0x42 => u16_bytes(s.voltage * 1000.0),
        0x46 => vec![clamp_u8(18.0 + 40.0)],
        0x49 => vec![clamp_u8(s.throttle * 255.0 / 100.0)],
        0x51 => vec![match fuel {
            Engine::Diesel => 4,
            _ => 1,
        }],
        0x5C => vec![clamp_u8(s.coolant - 4.0 + 40.0)],
        0x5E => u16_bytes(s.fuel_rate * 20.0),
        _ => return None,
    })
}
