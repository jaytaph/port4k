// Factions available
#[allow(unused)]
const FACTIONS: [&str; 5] = [
    "Aether Syndicate",
    "Forge Collective",
    "Voidborn",
    "Concord of Light",
    "Free Navigators",
];

pub struct Level {
    pub level: i32,
    pub name: &'static str,
    pub min_xp: u32,
}

const LEVELS: &[Level] = &[
    Level { level: 1,  name: "Drifter",              min_xp: 0 },
    Level { level: 2,  name: "Spacer",               min_xp: 200 },
    Level { level: 3,  name: "Rover",                min_xp: 500 },
    Level { level: 4,  name: "Wayfarer",             min_xp: 1000 },
    Level { level: 5,  name: "Explorer",             min_xp: 2000 },
    Level { level: 6,  name: "Pathfinder",           min_xp: 3500 },
    Level { level: 7,  name: "Voyager",              min_xp: 5500 },
    Level { level: 8,  name: "Trailblazer",          min_xp: 8500 },
    Level { level: 9,  name: "Pioneer",              min_xp: 12500 },
    Level { level: 10, name: "Starfarer",            min_xp: 18000 },
    Level { level: 11, name: "Void Walker",          min_xp: 25000 },
    Level { level: 12, name: "Nebula Runner",        min_xp: 35000 },
    Level { level: 13, name: "Cosmic Sage",          min_xp: 48000 },
    Level { level: 14, name: "Star Weaver",          min_xp: 65000 },
    Level { level: 15, name: "Constellation Master", min_xp: 87000 },
    Level { level: 16, name: "Stellar Architect",    min_xp: 115000 },
    Level { level: 17, name: "Nova Seeker",          min_xp: 150000 },
    Level { level: 18, name: "Dimensional Shifter",  min_xp: 195000 },
    Level { level: 19, name: "Celestial Wanderer",   min_xp: 250000 },
    Level { level: 20, name: "Eternal Voyager",      min_xp: 320000 },
];

pub fn xp_to_level(xp: u32) -> i32 {
    // Find the highest level whose min_xp <= xp
    LEVELS
        .iter()
        .rev()
        .find(|lvl| xp >= lvl.min_xp)
        .map(|lvl| lvl.level)
        .unwrap_or(1)
}

pub fn xp_to_level_name(xp: u32) -> String {
    LEVELS
        .iter()
        .rev()
        .find(|lvl| xp >= lvl.min_xp)
        .map(|lvl| lvl.name.to_string())
        .unwrap_or_else(|| "Unknown".to_string())
}
