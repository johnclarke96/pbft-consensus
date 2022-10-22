use clap::{crate_name, crate_version, App};
use std::{
    collections::HashMap,
    io::{Read, Write},
    net::{Shutdown, TcpListener, TcpStream},
    thread, time,
    sync::mpsc::{Sender, Receiver, channel}
};
use thiserror::Error;

pub type Port = u64;
pub type Digest = Vec<Vec<u8>>;
pub type Message = Vec<Vec<u8>>;

// constants
const CONNECTION_DELAY: u64 = 3000;
const CONNECTION_ATTEMPTS_MAX: u64 = 3;

fn main() {
    let matches = App::new(crate_name!())
        .version(crate_version!())
        .about("A research implementation of Practical Byzantine Fault Tolerance.")
        .args_from_usage("--local-port=<PORT> 'The port on which the node is listening for messages'")
        .args_from_usage("--remote-ports=<PORTS> 'A comma-delimited list of ports on which the remote nodes are listening for messages'")
        .get_matches();

    let local_port = matches
        .value_of("local-port")
        .unwrap()
        .parse::<Port>()
        .unwrap();
    let remote_ports: Vec<u64> = matches
        .value_of("remote-ports")
        .unwrap()
        .split(",")
        .map(|x: &str| x.parse::<Port>().unwrap())
        .collect();

    //let network_server: NetworkServer = NetworkServer::new(local_port);
    //let mut network_client: NetworkClient = NetworkClient::new(remote_ports);

    // loops over remote peers in network until has connected with all
    //network_client.attempt_connections();

    // spawn threads with connections
}

pub struct Node {
    network_client: NetworkClient,
    network_server: NetworkServer,
    buffer: HashMap<Digest, Message>,
}

impl Node {
    pub fn spawn(local_port: Port, remote_ports: Vec<Port>, rx_message: Receiver<Vec<u8>>) -> Vec<thread::JoinHandle<()>> {
        let server_handle = NetworkServer::spawn(local_port);
        let client_handle = NetworkClient::spawn(remote_ports, rx_message);
        vec![server_handle, client_handle]
    }
}

#[derive(Eq, PartialEq, Clone, Debug, Error, Hash)]
pub enum PBFTError {
    #[error("Client could not connect to server")]
    TcpConnectionError,
}

pub struct NetworkClient {
    remote_ports: Vec<Port>,
    connections: HashMap<Port, TcpStream>,
    rx_message: Receiver<Vec<u8>>
}

impl NetworkClient {
    pub fn default(remote_ports: Vec<Port>, rx_message: Receiver<Vec<u8>>) -> Self {
        let connections = HashMap::new();

        NetworkClient {
            remote_ports,
            connections,
            rx_message
        }
    }

    pub fn spawn(remote_ports: Vec<Port>, rx_message: Receiver<Vec<u8>>) -> thread::JoinHandle<()> {
        let connections: HashMap<Port, TcpStream> = HashMap::new();

        let client_handle = thread::spawn(move || {
            NetworkClient {
                remote_ports,
                connections,
                rx_message
            }
            .run()
        });
        client_handle
    }

    pub fn run(&mut self) {
        // attempt to get connections and save them to connections HashMap
        if self.attempt_connections().is_ok() {
            loop {
                while let Ok(message) = self.rx_message.recv() {
                    println!("{:?}", &message);
                    let remote_ports = self.remote_ports.clone();
                    for port in remote_ports {
                        self.send_message(port, &message);
                    }
                }
            }
        }
    }

    pub fn send_message(&mut self, port: Port, message: &Vec<u8>) {
        // NOTE: to modify a mutable value within a hashmap use get_mut
        let tcp_stream = self.connections.get_mut(&port).expect("Unable to get mutable reference to TcpStream");
        tcp_stream.write(message).expect("Failed to send message");
    }

    pub fn attempt_connections(&mut self) -> Result<(), PBFTError> {
        let mut connection_attempts = 0;
        loop {
            let mut successful_connections = 0;
            thread::sleep(time::Duration::from_millis(CONNECTION_DELAY));
            for port in &self.remote_ports {
                let remote_address = format!("{}{}", "localhost:", port);
                match TcpStream::connect(remote_address) {
                    Ok(connection) => {
                        self.connections.insert(*port, connection);
                        successful_connections += 1;
                    }
                    Err(e) => {
                        println!("{:?}", e);
                        break; // if can't connect to one port, just try the entire network of peers
                    }
                }
            }
            // if successfully connected to each port return Ok
            if successful_connections == self.remote_ports.len() {
                return Ok(());
            }
            connection_attempts += 1;
            if connection_attempts == CONNECTION_ATTEMPTS_MAX {
                return Err(PBFTError::TcpConnectionError);
            }
        }
    }
}

pub struct NetworkServer {
    port: Port,
}

impl NetworkServer {
    pub fn spawn(port: Port) -> thread::JoinHandle<()> {
        let server_handle = thread::spawn(move || NetworkServer { port }.run());
        server_handle
    }

    pub fn run(&self) {
        // create server binding
        let server = TcpListener::bind(format!("{}{}", "0.0.0.0:", self.port.to_string())).unwrap();
        println!("Server listening on port {}", self.port);
        for stream in server.incoming() {
            match stream {
                Ok(stream) => {
                    println!("My address: {}", stream.local_addr().unwrap());
                    println!("New connection: {}", stream.peer_addr().unwrap());
                    thread::spawn(move || {
                        // connection succeeded
                        Self::handle_client(stream)
                    });
                }
                Err(e) => {
                    println!("Error: {}", e);
                    /* connection failed */
                }
            }
        }
    }

    fn handle_client(mut stream: TcpStream) {
        let mut data = [0 as u8; 50]; // using 50 byte buffer
        while match stream.read(&mut data) {
            Ok(n) => {
                // echo everything!
                // TODO: because this is synchronous, the stream is continuously queried and 99.9% of the time is empty
                if n > 0 {
                    println!("Data output: {:?}", data);
                }
                true
            }
            Err(_) => {
                println!(
                    "An error occurred, terminating connection with {}",
                    stream.peer_addr().unwrap()
                );
                stream.shutdown(Shutdown::Both).unwrap();
                false
            }
        } {}
    }
}

#[cfg(test)]
pub mod main_tests {

    use crate::NetworkClient;
    use crate::NetworkServer;
    use crate::Node;
    use std::{
        collections::HashMap,
        io::{Write},
        net::TcpStream,
        thread, time,
        sync::mpsc::channel
    };
    pub type Port = u64;

    #[test]
    fn test_server_launch() {
        let port: Port = 9001;
        let _ = NetworkServer::spawn(port);
        // connect and send a message
        thread::spawn(move || {
            let remote_address = format!("{}{}", "127.0.0.1:", port);
            match TcpStream::connect(remote_address) {
                Ok(mut connection) => {
                    connection.write(b"Hello").unwrap();
                }
                Err(_) => {
                    panic!("Connection to server failed!");
                }
            }
        });

        thread::sleep(time::Duration::from_millis(2000));
    }

    #[test]
    fn test_client_launch() {
        // connect to ports successfully
        let ports: Vec<Port> = vec![9000, 9001, 9002, 9003];
        let _: Vec<thread::JoinHandle<()>> =
            ports.iter().map(|x| NetworkServer::spawn(*x)).collect();
        let (_, rx_message) = channel::<Vec<u8>>();
        let (_, rx1_message) = channel::<Vec<u8>>();
        let _: thread::JoinHandle<()> = NetworkClient::spawn(ports, rx_message);

        thread::sleep(time::Duration::from_millis(5000));

        let launch_ports = vec![9004];
        let remote_ports: Vec<Port> = vec![9004, 9005];
        let _: Vec<thread::JoinHandle<()>> = launch_ports
            .iter()
            .map(|x| NetworkServer::spawn(*x))
            .collect();
        let connections: HashMap<Port, TcpStream> = HashMap::new();

        assert!(NetworkClient {
            remote_ports,
            connections,
            rx_message: rx1_message
        }
        .attempt_connections()
        .is_err());
    }

    #[test]
    fn test_client_send_messages() {
        // connect to ports successfully
        let ports: Vec<Port> = vec![9000, 9001, 9002, 9003];
        let _: Vec<thread::JoinHandle<()>> =
            ports.iter().map(|x| NetworkServer::spawn(*x)).collect();
        let (_, rx_message) = channel::<Vec<u8>>();
        let mut network_client = NetworkClient::default(ports, rx_message);
        network_client.attempt_connections().unwrap();
        // make sure servers launch
        thread::sleep(time::Duration::from_millis(1000));
        // send message from client
        network_client.send_message(network_client.remote_ports[0], &vec![1]);
        // wait for response from server
        thread::sleep(time::Duration::from_millis(3000));
    }

    #[test]
    fn launch_node_and_send_msg() {
        let ports: Vec<Port> = vec![9000, 9001, 9002, 9003];
        let _: Vec<thread::JoinHandle<()>> =
            ports.iter().map(|x| NetworkServer::spawn(*x)).collect();
        let (tx_message, rx_message) = channel::<Vec<u8>>();
        let local_port: Port = 9004;
        let _ = Node::spawn(local_port, ports, rx_message);

        let msg: Vec<u8> = vec![1];
        tx_message.send(msg).unwrap();
        thread::sleep(time::Duration::from_millis(6000));
    }
}

/***
 *
 */
