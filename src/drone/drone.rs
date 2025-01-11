use crossbeam_channel::{select_biased, Receiver, Sender};
use rand::random_range;
use log::{info, debug, error, warn};
use std::collections::{HashMap, HashSet};

use wg_2024::{
    controller::{DroneCommand, DroneEvent},
    drone::Drone,
    network::{NodeId, SourceRoutingHeader},
    packet::{FloodRequest, Fragment, Nack, NackType, NodeType, Packet, PacketType},
};

pub struct KrustyCrapDrone {
    id: NodeId,                                     // Unique identifier for this drone
    controller_send: Sender<DroneEvent>,            // Channel for sending events to the controller
    controller_recv: Receiver<DroneCommand>,        // Channel for receiving commands from the controller
    packet_recv: Receiver<Packet>,                  // Channel for receiving packets from other nodes
    packet_send: HashMap<NodeId, Sender<Packet>>,   // Map of node IDs to their packet senders
    pdr: f32,                                       // Packet drop rate (0.0 to 1.0)
    floods: HashMap<NodeId, HashSet<u64>>,          // Tracks processed flood requests by initiator
    crashing_behavior: bool,                        // Indicates if drone is in crashing state
}

impl Drone for KrustyCrapDrone {
    /// ###### Creates a new instance of KrustyCrapDrone with the given parameters
    fn new(
        id: NodeId,
        controller_send: Sender<DroneEvent>,
        controller_recv: Receiver<DroneCommand>,
        packet_recv: Receiver<Packet>,
        packet_send: HashMap<NodeId, Sender<Packet>>,
        pdr: f32,
    ) -> Self {
        info!("Initializing new KrustyCrapDrone with ID: {}", id);
        Self {
            id,
            controller_send,
            controller_recv,
            packet_recv,
            packet_send,
            pdr,
            floods: HashMap::new(),
            crashing_behavior: false,
        }
    }

    /// ###### Starts the drone and enters the main loop
    fn run(&mut self) {
        info!("Starting KrustyCrapDrone with ID: {}", self.id);

        // Check for special PDR values first for and ester egg.
        self.ester_egg();

        loop {
            // Use biased select to prioritize controller commands
            select_biased! {
                recv(self.controller_recv) -> command => {
                    if let Ok(command) = command {
                        debug!("Drone {} received controller command: {:?}", self.id, command);
                        self.handle_command(command);
                    }
                }
                recv(self.packet_recv) -> packet => {
                    if let Ok(packet) = packet {
                        debug!("Drone {} received packet: {:?}", self.id, packet);
                        self.handle_packet(packet);
                    }
                },
            }
            // Break the loop if drone is crashing and no packets left to process
            if self.crashing_behavior && self.packet_recv.is_empty() {
                warn!("Drone {} crashed", self.id);
                break;
            }
        }
    }
}

impl KrustyCrapDrone {
    /// ###### Routes incoming packets to appropriate handlers based on packet type
    fn handle_packet(&mut self, packet: Packet) {
        match packet.pack_type {
            PacketType::Nack(_) => self.handle_nack(packet),
            PacketType::Ack(_) => self.handle_ack(packet),
            PacketType::MsgFragment(fragment) => self.handle_fragment(fragment, packet.routing_header, packet.session_id),
            PacketType::FloodRequest(flood_request) => self.handle_flood_request(flood_request, packet.session_id),
            PacketType::FloodResponse(_) => self.handle_flood_response(packet),
        }
    }

    /// ###### Processes controller commands for drone configuration
    fn handle_command(&mut self, command: DroneCommand) {
        match command {
            DroneCommand::AddSender(id, sender) => {
                info!("Drone {}: Adding sender for node {}", self.id, id);
                self.packet_send.insert(id, sender);
            },
            DroneCommand::RemoveSender(id) => {
                info!("Drone {}: Removing sender for node {}", self.id, id);
                self.packet_send.remove(&id);
            },
            DroneCommand::SetPacketDropRate(pdr) => {
                info!("Drone {}: Updating packet drop rate from {} to {}", self.id, self.pdr, pdr);
                self.pdr = pdr;
            },
            DroneCommand::Crash => {
                warn!("Drone {} received crash command", self.id);
                self.crashing_behavior = true;
            }
        }
    }

    /// ###### Forwards NACK packets to the next hop
    fn handle_nack(&mut self, packet: Packet) {
        debug!("Drone {}: Forwarding NACK packet", self.id);
        self.send_to_next_hop(packet);
    }

    /// ###### Forwards ACK packets to the next hop
    fn handle_ack(&mut self, packet: Packet) {
        debug!("Drone {}: Forwarding ACK packet", self.id);
        self.send_to_next_hop(packet);
    }

    /// ###### Processes message fragments and handles routing decisions
    fn handle_fragment(
        &mut self,
        fragment: Fragment,
        routing_header: SourceRoutingHeader,
        session_id: u64
    ) {
        debug!("Drone {}: Handling fragment {} for session {}", self.id, fragment.fragment_index, session_id);

        // Check if the drone is in a crashing state.
        // If so, send a Nack 'ErrorInRouting' with 'self.id'.
        if self.crashing_behavior {
            warn!("Drone {} is in crashing state, sending ErrorInRouting NACK", self.id);
            self.send_nack(NackType::ErrorInRouting(self.id), fragment.fragment_index, routing_header, session_id);
            return;
        }

        // Retrieve the current hop from the routing header.
        // If it doesn't exist, send a Nack 'UnexpectedRecipient' with 'self.id'.
        let Some(current_hop_id) = routing_header.current_hop() else {
            error!("Drone {}: No current hop in routing header, sending UnexpectedRecipient NACK", self.id);
            self.send_nack(NackType::UnexpectedRecipient(self.id), fragment.fragment_index, routing_header, session_id);
            return;
        };

        // If the current hop isn't the drone's ID, send a Nack 'UnexpectedRecipient' with 'self.id'.
        if self.id != current_hop_id {
            error!("Drone {}: Unexpected recipient ({}), sending NACK", self.id, current_hop_id);
            self.send_nack(NackType::UnexpectedRecipient(self.id), fragment.fragment_index, routing_header, session_id);
            return;
        }

        // Retrieve the next hop from the routing header.
        // If it doesn't exist, send a Nack 'DestinationIsDrone'.
        let Some(next_hop_id) = routing_header.next_hop() else {
            error!("Drone {}: No next hop in routing header, sending DestinationIsDrone NACK", self.id);
            self.send_nack(NackType::DestinationIsDrone, fragment.fragment_index, routing_header, session_id);
            return;
        };

        // Attempt to find the sender for the next hop.
        // If the sender isn't found, send a Nack 'ErrorInRouting' with next_hop_id.
        let Some(sender) = self.packet_send.get(&next_hop_id) else {
            error!("Drone {}: No sender found for next hop {}, sending ErrorInRouting NACK", self.id, next_hop_id);
            self.send_nack(NackType::ErrorInRouting(next_hop_id), fragment.fragment_index, routing_header, session_id);
            return;
        };

        // Create a new Fragment packet using the updated routing header, session ID and fragment
        let mut packet = Packet::new_fragment(routing_header.clone(), session_id, fragment.clone());

        // Simulate packet drop based on the PDR.
        // If the random number is less than PDR, send the 'PacketDropped' event to the simulation controller.
        // And send a Nack 'Dropped'.
        if random_range(0.0..1.0) <= self.pdr {
            warn!("Drone {}: Packet dropped due to PDR ({})", self.id, self.pdr);
            self.send_event(DroneEvent::PacketDropped(packet));
            self.send_nack(NackType::Dropped, fragment.fragment_index, routing_header, session_id);
            return;
        }

        // Increment the hop index in the routing header.
        packet.routing_header.increase_hop_index();

        // Attempt to send the updated fragment packet to the next hop.
        if sender.send(packet.clone()).is_err() {
            error!("Drone {}: Failed to send packet to next hop {}", self.id, next_hop_id);
        } else {
            info!("Drone {}: Successfully forwarded fragment {} to next hop {}", self.id, fragment.fragment_index, next_hop_id);
            self.send_event(DroneEvent::PacketSent(packet));
        }
    }

    /// ###### Sends NACK packet to the previous hop
    fn send_nack(&self, nack_type: NackType, fragment_index: u64, mut routing_header: SourceRoutingHeader, session_id: u64) {
        debug!("Drone {}: Sending NACK for fragment {} with type {:?}", self.id, fragment_index, nack_type);

        let nack = Nack {
            fragment_index,
            nack_type,
        };

        // Truncate the hops in the routing header up to the current hop index + 1, to include current hop.
        routing_header.hops.truncate(routing_header.hop_index + 1);
        // Reverse the routing header to indicate the Nack should go backward in the route
        routing_header.hops.reverse();
        // Reset the hop index to 0
        routing_header.hop_index = 0;

        let nack_packet = Packet::new_nack(routing_header.clone(), session_id, nack);
        self.send_to_next_hop(nack_packet);
    }

    /// ###### Processes flood requests and forwards to neighbors
    fn handle_flood_request(&mut self, mut flood_request: FloodRequest, session_id: u64) {
        debug!("Drone {}: Handling flood request {} from initiator {}", self.id, flood_request.flood_id, flood_request.initiator_id);

        // Check if the drone is in a crashing state.
        // If so, just return.
        if self.crashing_behavior {
            warn!("Drone {} is in crashing state, ignoring flood request", self.id);
            return;
        }

        // Add current drone to the flood request's path trace.
        flood_request.increment(self.id, NodeType::Drone);

        let flood_id = flood_request.flood_id;
        let initiator_id = flood_request.initiator_id;

        // Check if the flood ID has already been received from this flood initiator.
        if self.floods.get(&initiator_id).map_or(false, |ids| ids.contains(&flood_id)) {
            info!("Drone {}: Flood {} from initiator {} already processed, sending response", self.id, flood_id, initiator_id);
            // Generate and send the flood response.
            let response = flood_request.generate_response(session_id);
            self.send_to_next_hop(response);
            return;
        }

        // If Flood ID has not yet been received from this flood initiator.
        // Add the flood ID to the set of processed floods for this initiator.
        self.floods
            .entry(initiator_id)
            .or_insert_with(HashSet::new)
            .insert(flood_id);

        // Check if there's a previous node (sender) in the flood path.
        // If the sender isn't found, print an error.
        let Some(sender_id) = self.get_prev_node_id(&flood_request.path_trace) else {
            error!("Drone {}: No previous node found in flood path", self.id);
            return;
        };

        // Get all neighboring nodes except the sender.
        let neighbors = self.get_neighbors_except(sender_id);

        // If there are neighbors, forward the flood request to them
        if !neighbors.is_empty() {
            info!("Drone {}: Forwarding flood request", self.id);
            self.forward_flood_request(neighbors, flood_request, session_id);
        } else {
            // If no neighbors, generate and send a response instead.
            info!("Drone {}: No neighbors to forward flood request to, sending response", self.id);
            let response = flood_request.generate_response(session_id);
            self.send_to_next_hop(response);
        }
    }

    /// ###### Retrieves the previous node ID from the path trace
    fn get_prev_node_id(&self, path_trace: &Vec<(NodeId, NodeType)>) -> Option<NodeId> {
        debug!("Drone {}: Getting previous node ID from path trace: {:?}", self.id, path_trace);
        if path_trace.len() > 1 {
            let prev_id = path_trace[path_trace.len() - 2].0;
            debug!("Drone {}: Previous node ID found: {}", self.id, prev_id);
            return Some(prev_id);
        }
        None
    }

    /// ###### Retrieves neighbors except the one with the given ID
    fn get_neighbors_except(&self, exclude_id: NodeId) -> Vec<&Sender<Packet>> {
        self.packet_send
            .iter()
            .filter(|(&node_id, _)| node_id != exclude_id)
            .map(|(_, sender)| sender)
            .collect()
    }

    /// ###### Forwards flood request to neighbors
    fn forward_flood_request(
        &self,
        neighbors: Vec<&Sender<Packet>>,
        request: FloodRequest,
        session_id: u64)
    {
        for sender in neighbors {
            // Create an empty routing header, because this is unnecessary in flood requests.
            let routing_header = SourceRoutingHeader::empty_route();
            let packet = Packet::new_flood_request(routing_header, session_id, request.clone());

            // Attempt to forward the flood request to the neighbor.
            if sender.send(packet.clone()).is_err() {
                error!("Drone {}: Failed to forward flood request", self.id);
            } else {
                debug!("Drone {}: Flood request forwarded successfully", self.id);
                self.send_event(DroneEvent::PacketSent(packet));
            }
        }
    }

    /// ###### Forwards FloodResponse packets to the next hop
    fn handle_flood_response(&mut self, packet: Packet) {
        debug!("Drone {}: Forwarding flood response: {:?}", self.id, packet);
        self.send_to_next_hop(packet);
    }

    /// ###### Sends packet to the next hop
    fn send_to_next_hop(&self, mut packet: Packet) {
        debug!("Drone {}: Sending packet to next hop: {:?}", self.id, packet);

        // Attempt to find the sender for the next hop
        let sender = self.get_sender_of_next(packet.routing_header.clone());

        // Increment the hop index in the routing header.
        packet.routing_header.increase_hop_index();
        debug!("Drone {}: Increased hop index in routing header", self.id);

        // Send the packet to the next hop if the sender is found.
        // If the sender is not found, send the packet through the controller.
        match sender {
            None => {
                warn!("Drone {}: Sending packet through controller", self.id);
                self.send_event(DroneEvent::ControllerShortcut(packet));
            }
            Some(sender) => {
                if sender.send(packet.clone()).is_err() {
                    error!("Drone {}: Failed to send packet to next hop, sending packet through controller", self.id);
                    self.send_event(DroneEvent::ControllerShortcut(packet));
                } else {
                    info!("Drone {}: Successfully sent packet to next hop", self.id);
                    self.send_event(DroneEvent::PacketSent(packet));
                }
            }
        }
    }

    /// ###### Retrieves the sender for the next hop from the routing header
    fn get_sender_of_next(&self, routing_header: SourceRoutingHeader) -> Option<&Sender<Packet>> {
        debug!("Drone {}: Getting sender for next hop from routing header", self.id);

        // Attempt to retrieve the next hop ID from the routing header.
        // If it is missing, return `None` as there is no valid destination to send the packet to.
        let Some(next_hop_id) = routing_header.next_hop() else {
            error!("Drone {}: No next hop found in routing header", self.id);
            return None;
        };

        // Attempt to retrieve the sender for the next hop ID.
        let sender = self.packet_send.get(&next_hop_id);
        if sender.is_none() {
            error!("Drone {}: No sender found for next hop {}", self.id, next_hop_id);
        } else {
            debug!("Drone {}: Found sender for next hop {}", self.id, next_hop_id);
        }
        sender
    }

    /// ###### Sends events to the controller
    fn send_event(&self, event: DroneEvent) {
        match event {
            DroneEvent::PacketSent(packet) => {
                if self.controller_send.send(DroneEvent::PacketSent(packet.clone())).is_err() {
                    error!("Drone {}: Failed to send PacketSent event to controller", self.id);
                } else {
                    info!("Drone {}: Packet with the PacketSent event sent successfully: {:?}", self.id, packet);
                }
            }
            DroneEvent::PacketDropped(packet) => {
                if self.controller_send.send(DroneEvent::PacketDropped(packet.clone())).is_err() {
                    error!("Drone {}: Failed to send PacketDropped event to controller", self.id);
                } else {
                    warn!("Drone {}: Packet with the PacketDropped event sent successfully: {:?}", self.id, packet);
                }
            }
            DroneEvent::ControllerShortcut(packet) => {
                if self.controller_send.send(DroneEvent::ControllerShortcut(packet.clone())).is_err() {
                    error!("Drone {}: Failed to send ControllerShortcut event to controller", self.id);
                } else {
                    debug!("Drone {}: Packet sent through controller shortcut: {:?}", self.id, packet);
                }
            }
        }
    }

    /// ###### Checks for special PDR values and triggers ester eggs
    fn ester_egg(&mut self) {
        debug!("Drone {}: Checking special PDR values...", self.id);
        if self.pdr == 0.777 {
            info!("Drone {}: Angel mode activated 😇", self.id);
            self.pdr = 0.0;
        }
        if self.pdr == 0.666 {
            warn!("Drone {}: Devil mode activated 😈", self.id);
            self.pdr = 1.0;
        }
        if self.pdr == 0.360_360_360 {
            info!("Drone {}: Attempting backflip...", self.id);
            Self::request_to_do_a_backflip();
        }
    }

    /// ###### Requests the drone to do a backflip
    fn request_to_do_a_backflip() {
        println!("You really thought that I can do a backflip? Bruuuh, you got scammed");
    }
}