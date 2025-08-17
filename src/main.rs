use ads1x1x::{Ads1x1x, DataRate16Bit, FullScaleRange, ModeChangeError, TargetAddr, channel};
use chrono::{Local, Utc};
use influxdb::InfluxDbWriteable;
use influxdb::{Client, Timestamp};
use linux_embedded_hal::I2cdev;
use std::fs::File;
use std::io::Write;
use std::{thread, time};

#[async_std::main]
async fn main() {
    let i2cpath = std::env::var("I2C").unwrap_or("/dev/i2c-1".to_string());
    let dev = I2cdev::new(&i2cpath).unwrap();
    let addrpin = std::env::var("ADDR").unwrap_or("default".to_string());
    let address = match &*addrpin {
        "GND" | "gnd" | "0x48" | "48" => TargetAddr::Gnd,
        "VDD" | "vdd" | "0x49" | "49" => TargetAddr::Vdd,
        "SDA" | "sda" | "0x4A" | "4A" => TargetAddr::Sda,
        "SCL" | "scl" | "0x4B" | "4B" => TargetAddr::Scl,
        _ => TargetAddr::default(),
    };
    let adc = Ads1x1x::new_ads1115(dev, address);
    match adc.into_continuous() {
        Err(ModeChangeError::I2C(e, adc)) => {
            let _dev = adc.destroy_ads1115();
            panic!("{}", &e)
        }
        Ok(mut adc) => {
            let chan = std::env::var("CHANNEL").unwrap_or("default".to_string());
            match &*chan {
                "A0" => adc.select_channel(channel::SingleA0).unwrap(),
                "A1" => adc.select_channel(channel::SingleA1).unwrap(),
                "A2" => adc.select_channel(channel::SingleA2).unwrap(),
                "A3" => adc.select_channel(channel::SingleA3).unwrap(),
                "A0A1" => adc.select_channel(channel::DifferentialA0A1).unwrap(),
                "A0A3" => adc.select_channel(channel::DifferentialA0A3).unwrap(),
                "A1A3" => adc.select_channel(channel::DifferentialA1A3).unwrap(),
                "A2A4" => adc.select_channel(channel::DifferentialA2A3).unwrap(),
                _ => adc.select_channel(channel::SingleA0).unwrap(),
            }
            let sps = std::env::var("SPS").unwrap_or("default".to_string());
            let spsd;
            let spst: Vec<u32>;
            match &*sps {
                "860" => {
                    adc.set_data_rate(DataRate16Bit::Sps860).unwrap();
                    spsd = 860 / 2;
                    spst = (0..spsd).map(|i| 1000 * i / spsd).collect();
                }
                "475" => {
                    adc.set_data_rate(DataRate16Bit::Sps475).unwrap();
                    spsd = 475 / 2;
                    spst = (0..spsd).map(|i| 1000 * i / spsd).collect();
                }
                "250" => {
                    adc.set_data_rate(DataRate16Bit::Sps250).unwrap();
                    spsd = 250 / 2;
                    spst = (0..spsd).map(|i| 1000 * i / spsd).collect();
                }
                "128" => {
                    adc.set_data_rate(DataRate16Bit::Sps128).unwrap();
                    spsd = 128 / 2;
                    spst = (0..spsd).map(|i| 1000 * i / spsd).collect();
                }
                "64" => {
                    adc.set_data_rate(DataRate16Bit::Sps64).unwrap();
                    spsd = 64 / 2;
                    spst = (0..spsd).map(|i| 1000 * i / spsd).collect();
                }
                "32" => {
                    adc.set_data_rate(DataRate16Bit::Sps32).unwrap();
                    spsd = 32 / 2;
                    spst = (0..spsd).map(|i| 1000 * i / spsd).collect();
                }
                "16" => {
                    adc.set_data_rate(DataRate16Bit::Sps16).unwrap();
                    spsd = 16 / 2;
                    spst = (0..spsd).map(|i| 1000 * i / spsd).collect();
                }
                "8" => {
                    adc.set_data_rate(DataRate16Bit::Sps8).unwrap();
                    spsd = 8 / 2;
                    spst = (0..spsd).map(|i| 1000 * i / spsd).collect();
                }
                _ => {
                    adc.set_data_rate(DataRate16Bit::Sps860).unwrap();
                    spsd = 860 / 2;
                    spst = (0..spsd).map(|i| 1000 * i / spsd).collect();
                }
            }
            let scale = std::env::var("SCALE").unwrap_or("default".to_string());
            match &*scale {
                "6.144" => adc
                    .set_full_scale_range(FullScaleRange::Within6_144V)
                    .unwrap(),
                "4.096" => adc
                    .set_full_scale_range(FullScaleRange::Within4_096V)
                    .unwrap(),
                "2.048" => adc
                    .set_full_scale_range(FullScaleRange::Within2_048V)
                    .unwrap(),
                "1.024" => adc
                    .set_full_scale_range(FullScaleRange::Within1_024V)
                    .unwrap(),
                "0.512" => adc
                    .set_full_scale_range(FullScaleRange::Within0_512V)
                    .unwrap(),
                "0.256" => adc
                    .set_full_scale_range(FullScaleRange::Within0_256V)
                    .unwrap(),
                _ => adc
                    .set_full_scale_range(FullScaleRange::Within4_096V)
                    .unwrap(),
            }
            let mut values: i16;
            let measurement = std::env::var("MEASUREMENT").unwrap_or("ads1115".to_string());
            let field = std::env::var("FIELDS").unwrap_or("value".to_string());
            let dur = time::Duration::from_micros(500);
            let exporter = std::env::var("EXPORTER").unwrap_or("default".to_string());
            if exporter != "CSV" {
                let url = std::env::var("INFLUXURL").unwrap_or("http://127.0.0.1:8086".to_string());
                let database = std::env::var("INFLUXDB").unwrap_or("my-bucket".to_string());
                let token = std::env::var("INFLUXTOKEN").unwrap_or("my-admin-token".to_string());
                let client = Client::new(url, database).with_token(token);
                let mut write_query;
                while Local::now().timestamp_subsec_millis() != 0 {}
                loop {
                    while !spst.contains(&Local::now().timestamp_subsec_millis()) {}
                    values = adc.read().unwrap();
                    println!("{:+06}", values);
                    write_query = Timestamp::Milliseconds(Utc::now().timestamp_millis() as u128)
                        .into_query(&field)
                        .add_field(&measurement, values);
                    client.query(&write_query).await.unwrap();
                    println!("{:+06}", values);
                    thread::sleep(dur);
                }
            } else {
                let mut writer =
                    File::create(Local::now().format("%Y%m%d%H%M%S.csv").to_string()).unwrap();
                writeln!(&mut writer, "#group,false,false,false,false,true,true").unwrap();
                writeln!(
                    &mut writer,
                    "#datatype,string,long,dateTime:RFC3339,long,string,string"
                )
                .unwrap();
                writeln!(&mut writer, "#default,_result,,,,,").unwrap();
                writeln!(
                    &mut writer,
                    ",result,table,_time,_{},_field,_measurement",
                    field
                )
                .unwrap();
                while Local::now().timestamp_subsec_millis() != 0 {}
                loop {
                    while !spst.contains(&Local::now().timestamp_subsec_millis()) {}
                    values = adc.read().unwrap();
                    writeln!(
                        &mut writer,
                        ",,0,{},{:+06},{},{}",
                        Local::now().to_rfc3339(),
                        values,
                        field,
                        measurement
                    )
                    .unwrap();
                    println!("{:+06}", values);
                    thread::sleep(dur);
                }
            }

            #[allow(unreachable_code)]
            let _dev = adc.destroy_ads1115();
        }
    }
}
