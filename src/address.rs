use std::net::{IpAddr, Ipv4Addr, SocketAddr};

#[derive(Clone, Copy, PartialEq)]
pub struct Address {
    pub ip: IpAddr,
    pub port: u16,
}

impl Address {
    pub fn new(ip: IpAddr, port: u16) -> Address {
        return Address { ip, port };
    }

    pub fn here(port: u16) -> Address {
        return Address::new(IpAddr::V4(Ipv4Addr::UNSPECIFIED), port);
    }

    pub fn socket(&self) -> SocketAddr {
        return SocketAddr::new(self.ip, self.port);
    }

    pub fn to_text(&self) -> String {
        return self.socket().to_string();
    }

    pub fn from_text(text: &str) -> Option<Address> {
        let socket = text.parse::<SocketAddr>();
        if socket.is_err() {
            return None;
        }

        let socket = socket.unwrap();

        return Some(Address::new(socket.ip(), socket.port()));
    }
}