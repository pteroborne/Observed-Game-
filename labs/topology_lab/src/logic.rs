use super::model::{
    HallwayId, HallwayNode, Link, RoomNode, RoomType, SimpleRng, ThresholdEndpoint, ThresholdSlotId,
};
use observed_core::RoomId;
use std::collections::{HashMap, HashSet};

pub type ParsedTopology = (
    HashMap<RoomId, RoomNode>,
    HashMap<HallwayId, HallwayNode>,
    Vec<Link>,
);

/// Validates that the topology graph is a single connected component containing all defined nodes
/// and that there is a valid path from the Spawn room to the Exit room.
pub fn validate_connectivity(
    rooms: &HashMap<RoomId, RoomNode>,
    hallways: &HashMap<HallwayId, HallwayNode>,
    links: &[Link],
) -> Result<(), String> {
    // 1. Locate the Spawn Room
    let spawn_room = rooms
        .values()
        .find(|r| r.room_type == RoomType::Spawn)
        .ok_or_else(|| "Spawn room not found in layout".to_string())?;

    // 2. Perform Breadth-First Search (BFS) to find visited components
    let mut visited_rooms = HashSet::new();
    let mut visited_hallways = HashSet::new();
    let mut queue = Vec::new();

    visited_rooms.insert(spawn_room.id);
    queue.push(ThresholdEndpoint::Room(spawn_room.id, ThresholdSlotId(0))); // slot doesn't matter for node traversal

    while let Some(current) = queue.pop() {
        let links_to_visit: Vec<ThresholdEndpoint> = match current {
            ThresholdEndpoint::Room(room_id, _) => {
                // Find all links containing any slot of this room
                links
                    .iter()
                    .filter_map(|link| {
                        // Check if link connects to our room
                        if let ThresholdEndpoint::Room(r, _) = link.a
                            && r == room_id
                        {
                            Some(link.b)
                        } else if let ThresholdEndpoint::Room(r, _) = link.b
                            && r == room_id
                        {
                            Some(link.a)
                        } else {
                            None
                        }
                    })
                    .collect()
            }
            ThresholdEndpoint::Hallway(hall_id, _) => {
                // Find all links containing any slot of this hallway
                links
                    .iter()
                    .filter_map(|link| {
                        if let ThresholdEndpoint::Hallway(h, _) = link.a
                            && h == hall_id
                        {
                            Some(link.b)
                        } else if let ThresholdEndpoint::Hallway(h, _) = link.b
                            && h == hall_id
                        {
                            Some(link.a)
                        } else {
                            None
                        }
                    })
                    .collect()
            }
        };

        for next_ep in links_to_visit {
            match next_ep {
                ThresholdEndpoint::Room(r, _) => {
                    if visited_rooms.insert(r) {
                        queue.push(next_ep);
                    }
                }
                ThresholdEndpoint::Hallway(h, _) => {
                    if visited_hallways.insert(h) {
                        queue.push(next_ep);
                    }
                }
            }
        }
    }

    // 3. Assert all rooms and hallways are connected
    for &room_id in rooms.keys() {
        if !visited_rooms.contains(&room_id) {
            return Err(format!(
                "Room {} is cut off from the rest of the map",
                room_id.0
            ));
        }
    }
    for &hall_id in hallways.keys() {
        if !visited_hallways.contains(&hall_id) {
            return Err(format!(
                "Hallway {} is cut off from the rest of the map",
                hall_id.0
            ));
        }
    }

    // 4. Assert Exit room is visited
    let exit_visited = rooms
        .values()
        .filter(|r| r.room_type == RoomType::Exit)
        .all(|r| visited_rooms.contains(&r.id));
    if !exit_visited {
        return Err("Exit room is not reachable from the Spawn room".to_string());
    }

    Ok(())
}

/// ASCII Map Importer
pub fn parse_ascii_map(input: &str) -> Result<ParsedTopology, String> {
    let mut rooms = HashMap::new();
    let mut hallways = HashMap::new();
    let mut links = Vec::new();

    let mut section = "";

    for line in input.lines() {
        let trimmed = line.trim();
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        if trimmed == "ROOMS" || trimmed == "[ROOMS]" {
            section = "rooms";
            continue;
        } else if trimmed == "HALLWAYS" || trimmed == "[HALLWAYS]" {
            section = "hallways";
            continue;
        } else if trimmed == "CONNECTIONS" || trimmed == "[CONNECTIONS]" {
            section = "connections";
            continue;
        }

        match section {
            "rooms" => {
                // Format: ROOM 0: Spawn, Slots: [0, 1]
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() < 2 {
                    return Err(format!("Malformed Room line: {}", trimmed));
                }
                let room_str = parts[0].trim();
                let room_id_val: u32 = room_str
                    .strip_prefix("ROOM")
                    .or_else(|| room_str.strip_prefix("Room"))
                    .unwrap_or(room_str)
                    .trim()
                    .parse()
                    .map_err(|_| format!("Invalid Room ID: {}", room_str))?;

                let info_parts: Vec<&str> = parts[1].split(',').collect();
                let type_str = info_parts[0].trim();
                let room_type = RoomType::parse(type_str)
                    .ok_or_else(|| format!("Unknown Room Type: {}", type_str))?;

                let mut slots = HashSet::new();
                for info in &info_parts[1..] {
                    let info_trimmed = info.trim();
                    if let Some(slots_str) = info_trimmed.strip_prefix("Slots:") {
                        let slots_clean = slots_str.trim().replace(['[', ']'], "");
                        for s_str in slots_clean.split(';') {
                            let clean_s = s_str.trim();
                            if !clean_s.is_empty() {
                                let slot_val: u8 = clean_s
                                    .parse()
                                    .map_err(|_| format!("Invalid slot: {}", clean_s))?;
                                slots.insert(ThresholdSlotId(slot_val));
                            }
                        }
                    }
                }
                // Default slots if none specified
                if slots.is_empty() {
                    slots.insert(ThresholdSlotId(0));
                }

                let id = RoomId(room_id_val);
                rooms.insert(
                    id,
                    RoomNode {
                        id,
                        room_type,
                        slots,
                    },
                );
            }
            "hallways" => {
                // Format: HALLWAY 0: T-Junction, Slots: [0, 1, 2]
                let parts: Vec<&str> = trimmed.split(':').collect();
                if parts.len() < 2 {
                    return Err(format!("Malformed Hallway line: {}", trimmed));
                }
                let hall_str = parts[0].trim();
                let hall_id_val: u32 = hall_str
                    .strip_prefix("HALLWAY")
                    .or_else(|| hall_str.strip_prefix("Hallway"))
                    .unwrap_or(hall_str)
                    .trim()
                    .parse()
                    .map_err(|_| format!("Invalid Hallway ID: {}", hall_str))?;

                let info_parts: Vec<&str> = parts[1].split(',').collect();
                let name = info_parts[0].trim().to_string();

                let mut slots = HashSet::new();
                for info in &info_parts[1..] {
                    let info_trimmed = info.trim();
                    if let Some(slots_str) = info_trimmed.strip_prefix("Slots:") {
                        let slots_clean = slots_str.trim().replace(['[', ']'], "");
                        for s_str in slots_clean.split(';') {
                            let clean_s = s_str.trim();
                            if !clean_s.is_empty() {
                                let slot_val: u8 = clean_s
                                    .parse()
                                    .map_err(|_| format!("Invalid slot: {}", clean_s))?;
                                slots.insert(ThresholdSlotId(slot_val));
                            }
                        }
                    }
                }
                // Default slots if none specified
                if slots.is_empty() {
                    slots.insert(ThresholdSlotId(0));
                }

                let id = HallwayId(hall_id_val);
                hallways.insert(id, HallwayNode { id, name, slots });
            }
            "connections" => {
                // Format: ROOM 0, SLOT 0 <-> HALLWAY 0, SLOT 0
                // Or: HALLWAY 0, SLOT 2 <-> ROOM 1, SLOT 0
                let parts: Vec<&str> = trimmed.split("<->").collect();
                if parts.len() != 2 {
                    return Err(format!("Malformed Connection line: {}", trimmed));
                }

                let parse_ep = |s: &str| -> Result<ThresholdEndpoint, String> {
                    let s_trimmed = s.trim();
                    let ep_parts: Vec<&str> = s_trimmed.split(',').collect();
                    if ep_parts.len() != 2 {
                        return Err(format!("Invalid endpoint: {}", s_trimmed));
                    }
                    let node_str = ep_parts[0].trim();
                    let slot_str = ep_parts[1].trim();

                    let slot_val: u8 = slot_str
                        .strip_prefix("SLOT")
                        .or_else(|| slot_str.strip_prefix("Slot"))
                        .unwrap_or(slot_str)
                        .trim()
                        .parse()
                        .map_err(|_| format!("Invalid Slot: {}", slot_str))?;
                    let slot = ThresholdSlotId(slot_val);

                    if node_str.starts_with("ROOM") || node_str.starts_with("Room") {
                        let id_str = node_str
                            .strip_prefix("ROOM")
                            .or_else(|| node_str.strip_prefix("Room"))
                            .unwrap()
                            .trim();
                        let id_val: u32 = id_str
                            .parse()
                            .map_err(|_| format!("Invalid Room ID: {}", id_str))?;
                        Ok(ThresholdEndpoint::Room(RoomId(id_val), slot))
                    } else if node_str.starts_with("HALLWAY") || node_str.starts_with("Hallway") {
                        let id_str = node_str
                            .strip_prefix("HALLWAY")
                            .or_else(|| node_str.strip_prefix("Hallway"))
                            .unwrap()
                            .trim();
                        let id_val: u32 = id_str
                            .parse()
                            .map_err(|_| format!("Invalid Hallway ID: {}", id_str))?;
                        Ok(ThresholdEndpoint::Hallway(HallwayId(id_val), slot))
                    } else {
                        Err(format!("Unknown node type: {}", node_str))
                    }
                };

                let ep_a = parse_ep(parts[0])?;
                let ep_b = parse_ep(parts[1])?;

                links.push(Link { a: ep_a, b: ep_b });
            }
            _ => {}
        }
    }

    Ok((rooms, hallways, links))
}

/// Shuffles all unobserved threshold links, validating the resulting graph to make sure
/// connectivity and path solvability invariants are maintained.
pub fn shuffle_links(
    rooms: &HashMap<RoomId, RoomNode>,
    hallways: &HashMap<HallwayId, HallwayNode>,
    links: &mut Vec<Link>,
    observed_nodes: &HashSet<ThresholdEndpoint>,
    rng: &mut SimpleRng,
) -> bool {
    // 1. Separate frozen (observed) links from unobserved links
    let mut frozen_links = Vec::new();
    let mut unobserved_endpoints = Vec::new();

    for link in links.iter() {
        let is_a_observed = observed_nodes.iter().any(|&obs| match (obs, link.a) {
            (ThresholdEndpoint::Room(r1, _), ThresholdEndpoint::Room(r2, _)) if r1 == r2 => true,
            (ThresholdEndpoint::Hallway(h1, _), ThresholdEndpoint::Hallway(h2, _)) if h1 == h2 => {
                true
            }
            _ => false,
        });
        let is_b_observed = observed_nodes.iter().any(|&obs| match (obs, link.b) {
            (ThresholdEndpoint::Room(r1, _), ThresholdEndpoint::Room(r2, _)) if r1 == r2 => true,
            (ThresholdEndpoint::Hallway(h1, _), ThresholdEndpoint::Hallway(h2, _)) if h1 == h2 => {
                true
            }
            _ => false,
        });

        if is_a_observed || is_b_observed {
            frozen_links.push(*link);
        } else {
            unobserved_endpoints.push(link.a);
            unobserved_endpoints.push(link.b);
        }
    }

    if unobserved_endpoints.len() < 4 {
        // Not enough unobserved endpoints to shuffle meaningfully
        return false;
    }

    // 2. Perform validation trials for random shuffles
    for _trial in 0..100 {
        // Randomly pair unobserved endpoints
        let mut endpoints_copy = unobserved_endpoints.clone();
        rng.shuffle(&mut endpoints_copy);

        let mut trial_links = frozen_links.clone();
        for i in (0..endpoints_copy.len()).step_by(2) {
            if i + 1 < endpoints_copy.len() {
                trial_links.push(Link {
                    a: endpoints_copy[i],
                    b: endpoints_copy[i + 1],
                });
            }
        }

        // Validate the trial configuration
        if validate_connectivity(rooms, hallways, &trial_links).is_ok() {
            *links = trial_links;
            return true;
        }
    }

    false
}
