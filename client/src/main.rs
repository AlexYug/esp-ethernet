//! This example demonstrates how to configure an RMII based Ethernet adapter
//!
//! To use it, you need an RMII-capable Espressif MCU, like the original ESP32 chip

use heapless;
use anyhow::{bail, Result};
use core::str;
use esp_idf_svc::{
    eventloop::{
	EspSystemEventLoop,
	EspEventDeserializer,
	EspSubscription,
	EspEvent,
	System
    },
    wifi::WifiEvent,
    netif::IpEvent,
    eth,
    hal::{
	prelude::Peripherals,
	gpio,
    },
};

use core::time::Duration;
use std::sync::mpsc;
use esp_idf_hal::delay::Delay;


#[toml_cfg::toml_config]
pub struct Config {
    #[default("Wokwi-GUEST")]
    wifi_ssid: &'static str,
    #[default("")]
    wifi_psk: &'static str,
}

/// The relevant events for this example as it connects to the server,
/// sends a message, receives the same message, and closes the connection.
#[derive(Debug, PartialEq)]
enum MyEvent {
    IpAssigned,
}

fn a_callback(event: EspEvent) {
    let deserialized = <EspEvent<'_> as EspEventDeserializer>::deserialize(&event);
    log::info!("DEBUG ESP Event: {:?}", deserialized);
}

fn a_wifi_callback(event: WifiEvent) {
    log::info!("DEBUG WIFI Event: {:?}", event);
}

fn an_ip_callback(tx: &mpsc::Sender<MyEvent>, event: &IpEvent) {
    match event {
	IpEvent::DhcpIpAssigned(_)=>{
	    log::info!("DEBUG IP Event 3: {:?}", event);
	    let _ = tx.send(MyEvent::IpAssigned);
	},
	IpEvent::ApStaIpAssigned(_)=>{
	    log::info!("DEBUG IP Event 4: {:?}", event);
	},
	IpEvent::DhcpIpDeassigned(_)=>{
	    log::info!("DEBUG IP Event 5: {:?}", event);
	},
	IpEvent::DhcpIp6Assigned(_)=>{
	    panic!("Don't implemented");
	}
    };
}

fn an_eth_callback(event: eth::EthEvent) {
    log::info!("DEBUG ETH Event: {:?}", event);
}

fn main() -> Result<()> {
    esp_idf_svc::sys::link_patches();
    esp_idf_svc::log::EspLogger::initialize_default();

    let peripherals = match Peripherals::take() {
	Ok(inner) => inner,
	Err(error) => {
	    panic!("Unable to get 'Peripherals' {:?}", error);
	}
    };

    let pins = peripherals.pins;

    let sysloop = EspSystemEventLoop::take()?;

    let _: EspSubscription<System> = match sysloop.subscribe::<EspEvent, _>(a_callback) {
	Ok(inner) => {
	    inner
	},
	Err(err) => {
            bail!("Unable to create subscription: {:?}", err)
        }
    };

    let mut _wifi_subscription: EspSubscription<System> = match sysloop.subscribe::<WifiEvent, _>(a_wifi_callback) {
	Ok(inner) => {
	    inner
	},
	Err(err) => {
            bail!("Unable to create wifi subscription: {:?}", err)
        }
    };

    let (tx, rx) = mpsc::channel::<MyEvent>();
    let a_tx = tx.clone();
    let _: EspSubscription<System> = match unsafe {sysloop.subscribe_nonstatic::<IpEvent, _>(move |event: IpEvent| { an_ip_callback(&a_tx, &event); })} {
	Ok(inner) => {
	    inner
	},
	Err(err) => {
	    bail!("Unable to create ip subscription: {:?}", err)
        }
    };
    let mut _eth_subscription: EspSubscription<System> = match sysloop.subscribe::<eth::EthEvent, _>(an_eth_callback) {
	Ok(inner) => {
	    inner
	},
	Err(err) => {
            bail!("Unable to create eth subscription: {:?}", err)
        }
    };
    log::info!("DEBUG 7");
    
    let rmii_config = esp_idf_svc::eth::RmiiClockConfig::<gpio::Gpio0, gpio::Gpio16, gpio::Gpio17>::OutputGpio0(
            pins.gpio0,
        );

    let delay = Delay::new_default();                                                                                        
    let mut ip_config:  esp_idf_svc::netif::NetifConfiguration = esp_idf_svc::netif::NetifConfiguration::eth_default_client();
    ip_config.stack = esp_idf_svc::netif::NetifStack::Eth;
    let mut hostname: heapless::String<30> = heapless::String::new();
    let _ = hostname.push_str("a.cum.uz");
    let mut key: heapless::String<32> = heapless::String::new();
    let _ = key.push_str("eth1");
    ip_config.key = key;
    let mut description: heapless::String<8> = heapless::String::new();
    let _ = description.push_str("desceth1");
    ip_config.description = description;
    ip_config.ip_configuration = esp_idf_svc::ipv4::Configuration::Client(
	esp_idf_svc::ipv4::ClientConfiguration::DHCP(
	    esp_idf_svc::ipv4::DHCPClientSettings{
		hostname: Some(hostname),
	    }
	)
    );

    ip_config.custom_mac = Some([0x79, 0xe4, 0x23, 0xd4, 0x44, 0x12]);
    ip_config.stack = esp_idf_svc::netif::NetifStack::Eth;
    log::info!("DEBUG 9");
    let net_if = match esp_idf_svc::netif::EspNetif::new_with_conf(&ip_config) {
	Ok(inner) => {
	    inner
	},
	Err(err) => {
	    log::error!("Unable to create netif instance 0: {:?}", err);
	    bail!("Unable to create netif instance: {:?}", err)
        }
    };
    log::info!("DEBUG 10");

    let eth_driver:eth::EthDriver<esp_idf_svc::eth::RmiiEth> = match eth::EthDriver::new_rmii(
	peripherals.mac,
	pins.gpio25,
        pins.gpio26,
        pins.gpio27,
        pins.gpio23,
        pins.gpio22,
        pins.gpio21,
        pins.gpio19,
        pins.gpio18,
	rmii_config,
	Some(pins.gpio5),
	esp_idf_svc::eth::RmiiEthChipset::LAN87XX,
	Some(1),
	sysloop.clone(),
    ) {
	Ok(inner) => {
	    inner
	},
	Err(err) => {
	    log::error!("Unable to create eth device (0): {:?}", err);
	    bail!("Unable to create eth device: {:?}", err)
        }
    };
    log::info!("DEBUG 12");
    
    let ethernet: eth::EspEth<esp_idf_svc::eth::RmiiEth> = match eth::EspEth::wrap_all(
	eth_driver,
	net_if,
    )  {
	Ok(inner) => {
	    inner
	},
	Err(err) => {
	    log::error!("Unable to start eth driver 2(0): {:?}", err);
	    bail!("Unable to start eth driver 2: {:?}", err)
        }
    };
    log::error!("DEBUG 13");

    let mut ethernet = match esp_idf_svc::eth::BlockingEth::wrap(ethernet, sysloop) {
	Ok(inner) => {
	    inner
	},
	Err(err) => {
	    log::error!("Unable to get blocking eth (0): {:?}", err);
	    bail!("Unable to get blocking eth: {:?}", err)
        }
    };
    log::error!("DEBUG 14:");
    
    match ethernet.start() {
	Ok(_) => {},
	Err(err) => {
	    log::error!("Unable to connect to witred network (0): {:?}", err);
	    bail!("Unable to connect to witred network: {:?}", err)
        }
    };

    log::error!("DEBUG 15");
    
    match ethernet.wait_netif_up() {
	Ok(_) => {},
	Err(err) => {
	    log::error!("Unable to get IP by DHCP (0): {:?}", err);
	    bail!("Unable to get IP by DHCP: {:?}", err)
        }
    };

    log::info!("DEBUG 16");
    {

	let interface_index = ethernet.eth().netif().get_index();
	log::error!("PING on interfece {:?}", interface_index);

	
	let mut pinger = esp_idf_svc::ping::EspPing::new(interface_index);
	let ping_config = esp_idf_svc::ping::Configuration{
	    count: 10,
	    interval: Duration::from_secs(1),
	    timeout: Duration::from_secs(10),
	    data_size: 32,
	    tos: 0,
	};
	match pinger.ping(
	    esp_idf_svc::ipv4::Ipv4Addr::new(192, 168, 88, 1),
	    &ping_config,
	){
	    Ok(value) => {
		log::error!("Ping value {:?}", value);
	    },
	    Err(err) => {
		log::error!("Unable to connect to witred network (0): {:?}", err);
		bail!("Unable to connect to witred network: {:?}", err)
            }
	};
	
    }

    delay.delay_ms(20000);                                                            
    log::info!("DEBUG 21 {:?}", match ethernet.eth().netif().get_ip_info()
	       {
		   Ok(inner) => {
		       inner
		   },
		   Err(err) => {
		       bail!("Could get IP: {:?}", err)
		   }
	       });

    loop {
	match rx.recv_timeout(Duration::from_secs(5)) {
	    Ok(MyEvent::IpAssigned) => {
	    },
	    Err(error) => {
		panic!("Error during connect to wifi : {:?}", error);
	    }
	};
	log::info!("DEBUG 37");
    }
}
