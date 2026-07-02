use observed_core::RoomId;
use std::collections::HashSet;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct HallwayId(pub u32);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct ThresholdSlotId(pub u8);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum RoomType {
    Spawn,
    Exit,
    Key,
    VideoDisplay,
    Normal,
}

impl RoomType {
    pub fn parse(s: &str) -> Option<Self> {
        match s.trim().to_lowercase().as_str() {
            "spawn" => Some(RoomType::Spawn),
            "exit" => Some(RoomType::Exit),
            "key" => Some(RoomType::Key),
            "videodisplay" | "video_display" => Some(RoomType::VideoDisplay),
            "normal" => Some(RoomType::Normal),
            _ => None,
        }
    }
}

#[derive(Clone, Debug)]
pub struct RoomNode {
    pub id: RoomId,
    pub room_type: RoomType,
    pub slots: HashSet<ThresholdSlotId>,
}

#[derive(Clone, Debug)]
pub struct HallwayNode {
    pub id: HallwayId,
    pub name: String,
    pub slots: HashSet<ThresholdSlotId>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ThresholdEndpoint {
    Room(RoomId, ThresholdSlotId),
    Hallway(HallwayId, ThresholdSlotId),
}

impl ThresholdEndpoint {
    pub fn is_room(&self) -> bool {
        matches!(self, ThresholdEndpoint::Room(_, _))
    }

    pub fn is_hallway(&self) -> bool {
        matches!(self, ThresholdEndpoint::Hallway(_, _))
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Link {
    pub a: ThresholdEndpoint,
    pub b: ThresholdEndpoint,
}

impl Link {
    pub fn contains_node(&self, ep: ThresholdEndpoint) -> bool {
        self.a == ep || self.b == ep
    }

    pub fn other_endpoint(&self, ep: ThresholdEndpoint) -> Option<ThresholdEndpoint> {
        if self.a == ep {
            Some(self.b)
        } else if self.b == ep {
            Some(self.a)
        } else {
            None
        }
    }
}

/// A deterministic seeded PRNG (xorshift32) for shuffling in tests and the lab.
pub struct SimpleRng(pub u32);
impl SimpleRng {
    pub fn next_u32(&mut self) -> u32 {
        let mut x = self.0;
        x ^= x << 13;
        x ^= x >> 17;
        x ^= x << 5;
        self.0 = x;
        x
    }

    pub fn choose<T: Clone>(&mut self, slice: &[T]) -> Option<T> {
        if slice.is_empty() {
            None
        } else {
            let idx = (self.next_u32() as usize) % slice.len();
            Some(slice[idx].clone())
        }
    }

    pub fn shuffle<T>(&mut self, slice: &mut [T]) {
        for i in (1..slice.len()).rev() {
            let j = (self.next_u32() as usize) % (i + 1);
            slice.swap(i, j);
        }
    }
}
