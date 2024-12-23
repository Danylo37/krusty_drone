# KrustyCrapDrone

### Import it as a library, and it's ready to use!!
_Cargo.toml_
```toml
[dependencies]
krusty_drone = { git = "https://github.com/Danylo37/krusty_drone.git"}
```
## Overview
The `KrustyCrapDrone` is a Rust-based drone implementation designed to manage communication and routing tasks in a simulated network. This implementation integrates with the `wg_2024` library, utilizing various components like packet handling, flood routing, and drone command execution.

## Features
- **Packet Handling**: Processes multiple packet types, including acknowledgments, fragments, and flood requests.
- **Flood Routing**: Implements flood routing with response handling and neighbor communication.
- **Command Processing**: Handles drone commands like adding/removing senders, setting packet drop rates, and crashing.
- **Error Handling**: Sends acknowledgments (NACKs) for various routing errors.
- **Simulation Integration**: Communicates with a simulation controller via events like `PacketSent` and `PacketDropped`.

## Dependencies
The drone relies on the following external crates and modules:
- `crossbeam_channel`: For asynchronous communication between components.
- `rand`: For generating random values, used in simulating packet drop rates.
- `wg_2024` library: Provides essential components such as `Packet`, `DroneCommand`, `DroneEvent`, and routing utilities.

## Code Structure
### Fields
- `id`: Unique identifier for the drone.
- `controller_send`: Channel to send events to the simulation controller.
- `controller_recv`: Channel to receive commands from the controller.
- `packet_recv`: Channel to receive incoming packets.
- `packet_send`: Map of neighboring nodes and their respective senders.
- `pdr`: Packet Drop Rate, used for simulating packet losses.
- `floods`: Tracks flood IDs.
- `crashing_behavior`: Flag to simulate a crashing drone.

### Core Functions
#### Constructor
```rust
fn new(
    id: NodeId,
    controller_send: Sender<DroneEvent>,
    controller_recv: Receiver<DroneCommand>,
    packet_recv: Receiver<Packet>,
    packet_send: HashMap<NodeId, Sender<Packet>>,
    pdr: f32,
) -> Self
```
Initializes a new drone instance with the given parameters.

#### Main Loop
```rust
fn run(&mut self)
```
Listens for incoming packets and controller commands, handling them accordingly.


### Packet Handling
- `handle_packet`: Handles various packet types (e.g., NACK, ACK, fragments, flood requests/responses).

### Command Handling
It's the part that handles the commands from the simulation controller.
- `handle_command`: Processes commands like adding/removing senders, adjusting PDR, and simulating crashes.

### Flood Routing 
- `handle_flood_request`: Manages incoming flood requests, forwarding them to neighbors or generating responses.

- `handle_flood_response`: Forwards flood responses to the next hop.

### Simulation Integration 
- `send_event`: Sends simulation events (e.g. `PacketSent`, `PackedDropped`) to the simulation controller
- `request_to_do_a_backflip`: icing on the cake that makes our drone unique.

## Costumer support: 
Telegram support:
- Mr. Krab: @lilloLeo
- Krusty Crap HeadChef: @NooXiii 
- Krusty Technicians: @olzhkkk @chenlei2003

**PS**: Hopefully we will put our mails
