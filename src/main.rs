#![allow(non_snake_case)]

mod menu;
mod mobmodel;
mod mobs;

use menu::GameState;
use glutin::prelude::*;
use glutin::config::ConfigTemplateBuilder;
use glutin::context::ContextAttributesBuilder;
use glutin::display::GetGlDisplay;
use glutin::surface::WindowSurface;
use glutin_winit::DisplayBuilder;
use raw_window_handle::HasRawWindowHandle;
use std::num::NonZeroU32;
use std::collections::{HashSet, HashMap};
use std::fs::File;
use std::io::{BufReader, Read, Write, Seek, SeekFrom};
use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Sender, Receiver};
use std::thread;
use winit::event::{Event, WindowEvent, ElementState, KeyEvent, MouseButton, MouseScrollDelta};
use winit::keyboard::{PhysicalKey, KeyCode};
use winit::event_loop::EventLoop;
use winit::window::WindowBuilder;
use glow::HasContext;
use glam::{Vec3, Mat4};
use image::GenericImageView;
use flate2::Compression;
use flate2::read::ZlibDecoder;
use flate2::write::{ZlibEncoder, GzEncoder};
use serde::{Serialize, Deserialize};
use fastnbt::ByteArray;
use noise::{NoiseFn, Perlin};
use rodio::{OutputStream, Source};

use mobs::{Mob, MobType, Ctx, Ev};

#[repr(C)]
#[derive(Copy, Clone, bytemuck::Pod, bytemuck::Zeroable)]
struct Vertex {
    position: [f32; 3],
    uv: [f32; 2],
}

const CHUNK_SIZE_X: usize = 16;
const CHUNK_SIZE_Z: usize = 16;
const CHUNK_SIZE_Y: usize = 128;

type Block = u8;

#[derive(Serialize, Deserialize)]
struct EmptyCompound {}

#[derive(Serialize, Deserialize)]
struct ChunkNbtLevel {
    xPos: i32,
    zPos: i32,
    LastUpdate: i64,
    TerrainPopulated: i8,
    Blocks: ByteArray,
    Data: ByteArray,
    SkyLight: ByteArray,
    BlockLight: ByteArray,
    HeightMap: ByteArray,
    Entities: Vec<EmptyCompound>,
    TileEntities: Vec<EmptyCompound>,
}

#[derive(Serialize, Deserialize)]
struct ChunkNbtRoot {
    Level: ChunkNbtLevel,
}

#[derive(Serialize, Deserialize, Debug)]
struct PlayerData {
    Pos: Option<Vec<f64>>,
    Rotation: Option<Vec<f32>>,
}

#[derive(Serialize, Deserialize, Debug)]
struct WorldData {
    SpawnX: Option<i32>,
    SpawnY: Option<i32>,
    SpawnZ: Option<i32>,
    Player: Option<PlayerData>,
}

#[derive(Serialize, Deserialize, Debug)]
struct LevelRoot {
    Data: WorldData,
}

#[derive(Serialize, Deserialize)]
struct PlayerSaveData {
    Pos: Vec<f64>,
    Motion: Vec<f64>,
    Rotation: Vec<f32>,
    FallDistance: f32,
    Fire: i16,
    Air: i16,
    OnGround: i8,
    Dimension: i32,
}

#[derive(Serialize, Deserialize)]
struct WorldSaveData {
    RandomSeed: i64,
    SpawnX: i32,
    SpawnY: i32,
    SpawnZ: i32,
    Time: i64,
    SizeOnDisk: i64,
    LastPlayed: i64,
    Player: PlayerSaveData,
    LevelName: String,
    version: i32,
    raining: i8,
    rainTime: i32,
    thundering: i8,
    thunderTime: i32,
}

#[derive(Serialize, Deserialize)]
struct LevelSaveRoot {
    Data: WorldSaveData,
}

#[derive(Clone)]
struct Chunk {
    blocks: [[[Block; CHUNK_SIZE_Y]; CHUNK_SIZE_Z]; CHUNK_SIZE_X],
    chunk_x: i32,
    chunk_z: i32,
}

struct GpuChunk {
    vao: glow::VertexArray,
    vbo: glow::Buffer,
    num_vertices: i32,
}

struct SoundManager {
    stream_handle: rodio::OutputStreamHandle,
    _stream: OutputStream,
}

impl SoundManager {
    fn new() -> Option<Self> {
        let (stream, stream_handle) = OutputStream::try_default().ok()?;
        Some(Self { stream_handle, _stream: stream })
    }

    fn play_sound(&self, path: &str) {
        if let Ok(file) = File::open(path) {
            if let Ok(source) = rodio::Decoder::new(BufReader::new(file)) {
                let _ = self.stream_handle.play_raw(source.convert_samples());
            }
        }
    }

    fn play_step(&self, block_id: Block) {
        let sound_path = match block_id {
            2 => "assets/sound/step/grass1.ogg",
            12 => "assets/sound/step/sand1.ogg",
            13 => "assets/sound/step/gravel1.ogg",
            17 | 18 => "assets/sound/step/wood1.ogg",
            _ => "assets/sound/step/stone1.ogg",
        };
        self.play_sound(sound_path);
    }

    fn play_dig(&self, block_id: Block) {
        let sound_path = match block_id {
            2 => "assets/sound/dig/grass1.ogg",
            12 => "assets/sound/dig/sand1.ogg",
            13 => "assets/sound/dig/gravel1.ogg",
            17 | 18 => "assets/sound/dig/wood1.ogg",
            _ => "assets/sound/dig/stone1.ogg",
        };
        self.play_sound(sound_path);
    }
}

fn get_minecraft_dir() -> Option<PathBuf> {
    if cfg!(target_os = "windows") {
        std::env::var_os("APPDATA").map(|val| PathBuf::from(val).join(".minecraft"))
    } else if cfg!(target_os = "macos") {
        std::env::var_os("HOME").map(|val| PathBuf::from(val).join("Library/Application Support/minecraft"))
    } else {
        std::env::var_os("HOME").map(|val| PathBuf::from(val).join(".minecraft"))
    }
}

fn extract_assets_from_jar() {
    let Some(mc_dir) = get_minecraft_dir() else { return; };
    let jar_path = mc_dir.join("versions/b1.7.3/b1.7.3.jar");

    if !jar_path.exists() {
        println!("Nie znaleziono pliku b1.7.3.jar w: {:?}", jar_path);
        return;
    }

    let file = match File::open(&jar_path) {
        Ok(f) => f,
        Err(_) => return,
    };

    let mut archive = match zip::ZipArchive::new(file) {
        Ok(a) => a,
        Err(_) => return,
    };

    let terrain_dest = Path::new("src/terrain.png");
    if !terrain_dest.exists() {
        for i in 0..archive.len() {
            let mut z_file = match archive.by_index(i) {
                Ok(f) => f,
                Err(_) => continue,
            };
            if z_file.name() == "terrain.png" {
                let mut buffer = Vec::new();
                if z_file.read_to_end(&mut buffer).is_ok() {
                    let _ = std::fs::write(terrain_dest, &buffer);
                    println!("Wypakowano terrain.png do src/terrain.png!");
                }
                break;
            }
        }
    }

    let sound_dir = Path::new("assets/sound");
    if !sound_dir.exists() {
        let _ = std::fs::create_dir_all(sound_dir);
        println!("Wypakowywanie dźwięków i mobów z b1.7.3.jar...");

        for i in 0..archive.len() {
            let mut z_file = match archive.by_index(i) {
                Ok(f) => f,
                Err(_) => continue,
            };
            let name = z_file.name().to_string();
            if name.starts_with("sound/") || name.starts_with("mob/") {
                let out_path = Path::new("assets").join(&name);
                if name.ends_with('/') {
                    let _ = std::fs::create_dir_all(&out_path);
                } else {
                    if let Some(p) = out_path.parent() {
                        let _ = std::fs::create_dir_all(p);
                    }
                    let mut outfile = match File::create(&out_path) {
                        Ok(f) => f,
                        Err(_) => continue,
                    };
                    let mut buffer = Vec::new();
                    if z_file.read_to_end(&mut buffer).is_ok() {
                        let _ = outfile.write_all(&buffer);
                    }
                }
            }
        }
        println!("Zasoby (dźwięki i moby) zostały pomyślnie wypakowane!");
    }
}

fn load_player_spawn_from_level(world_dir: &str) -> (Vec3, f32, f32) {
    let level_path = format!("{}/level.dat", world_dir);
    let default_pos = Vec3::new(8.0, 68.0, 8.0);
    let default_yaw = -90.0;
    let default_pitch = 0.0;

    let Ok(file_bytes) = std::fs::read(&level_path) else {
        return (default_pos, default_yaw, default_pitch);
    };

    let mut decoder = flate2::read::GzDecoder::new(&file_bytes[..]);
    let mut nbt_bytes = Vec::new();
    if decoder.read_to_end(&mut nbt_bytes).is_err() {
        return (default_pos, default_yaw, default_pitch);
    }

    let root: LevelRoot = match fastnbt::from_bytes(&nbt_bytes) {
        Ok(r) => r,
        Err(_) => return (default_pos, default_yaw, default_pitch),
    };

    let pos = if let Some(player) = root.Data.Player {
        if let Some(p) = player.Pos {
            if p.len() >= 3 {
                Vec3::new(p[0] as f32, p[1] as f32, p[2] as f32)
            } else {
                default_pos
            }
        } else {
            default_pos
        }
    } else {
        let sx = root.Data.SpawnX.unwrap_or(8) as f32;
        let sy = root.Data.SpawnY.unwrap_or(68) as f32;
        let sz = root.Data.SpawnZ.unwrap_or(8) as f32;
        Vec3::new(sx, sy, sz)
    };

    println!("Wczytano pozycję gracza ze świata: {:?}", pos);
    (pos, default_yaw, default_pitch)
}

fn save_player_data_to_level(world_dir: &str, player_pos: Vec3, yaw: f32, pitch: f32) {
    let level_path = format!("{}/level.dat", world_dir);

    let player_data = PlayerSaveData {
        Pos: vec![player_pos.x as f64, player_pos.y as f64, player_pos.z as f64],
        Motion: vec![0.0, 0.0, 0.0],
        Rotation: vec![yaw, pitch],
        FallDistance: 0.0,
        Fire: 0,
        Air: 300,
        OnGround: 1,
        Dimension: 0,
    };

    let world_data = WorldSaveData {
        RandomSeed: 1337,
        SpawnX: 8,
        SpawnY: 68,
        SpawnZ: 8,
        Time: 0,
        SizeOnDisk: 0,
        LastPlayed: std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_millis() as i64,
        Player: player_data,
        LevelName: "OpenMCWorld".to_string(),
        version: 19132,
        raining: 0,
        rainTime: 0,
        thundering: 0,
        thunderTime: 0,
    };

    let root = LevelSaveRoot { Data: world_data };

    let Ok(nbt_bytes) = fastnbt::to_bytes(&root) else {
        println!("Nie udało się zserializować danych gracza do NBT!");
        return;
    };

    let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
    if encoder.write_all(&nbt_bytes).is_err() { return; }
    let Ok(compressed) = encoder.finish() else { return; };

    if std::fs::write(&level_path, compressed).is_ok() {
        println!("Pomyślnie zapisano pozycję gracza w level.dat!");
    }
}

impl Chunk {
    fn new_generated(chunk_x: i32, chunk_z: i32) -> Self {
        let mut blocks = [[[0; CHUNK_SIZE_Y]; CHUNK_SIZE_Z]; CHUNK_SIZE_X];

        let perlin_1 = Perlin::new(1337);
        let perlin_2 = Perlin::new(7331);
        let detail_perlin = Perlin::new(42);

        for x in 0..CHUNK_SIZE_X {
            for z in 0..CHUNK_SIZE_Z {
                let world_x = chunk_x * 16 + x as i32;
                let world_z = chunk_z * 16 + z as i32;

                let fx = world_x as f64;
                let fz = world_z as f64;

                let noise_base = perlin_1.get([fx * 0.01, fz * 0.01]) * 10.0;
                let noise_hill = perlin_2.get([fx * 0.03, fz * 0.03]) * 20.0;
                let noise_detail = detail_perlin.get([fx * 0.08, fz * 0.08]) * 4.0;

                let height_f = 64.0 + noise_base + noise_hill + noise_detail;
                let surface_y = (height_f as usize).clamp(1, CHUNK_SIZE_Y - 6);

                for y in 0..=surface_y {
                    if y == 0 {
                        blocks[x][z][y] = 7;
                    } else if y == surface_y {
                        if surface_y < 60 {
                            blocks[x][z][y] = 12;
                        } else {
                            blocks[x][z][y] = 2;
                        }
                    } else if y >= surface_y.saturating_sub(3) {
                        blocks[x][z][y] = 3;
                    } else {
                        let ore_seed = (world_x.wrapping_mul(71) ^ y as i32 * 37 ^ world_z.wrapping_mul(101)) & 0xFF;
                        if ore_seed == 3 && y < 30 {
                            blocks[x][z][y] = 56;
                        } else if ore_seed % 13 == 0 && y < 50 {
                            blocks[x][z][y] = 14;
                        } else if ore_seed % 7 == 0 && y < 60 {
                            blocks[x][z][y] = 15;
                        } else if ore_seed % 5 == 0 && y < 64 {
                            blocks[x][z][y] = 16;
                        } else {
                            blocks[x][z][y] = 1;
                        }
                    }
                }

                for y in (surface_y + 1)..=60 {
                    blocks[x][z][y] = 9;
                }

                if surface_y >= 60 && x > 2 && x < 13 && z > 2 && z < 13 {
                    let pseudo_rand = ((world_x.wrapping_mul(31) ^ world_z.wrapping_mul(17)) & 0xFF) as u8;
                    if pseudo_rand < 4 {
                        let tree_height = 4;
                        for ty in 1..=tree_height {
                            if surface_y + ty < CHUNK_SIZE_Y {
                                blocks[x][z][surface_y + ty] = 17;
                            }
                        }
                        for lx in (x as i32 - 1)..=(x as i32 + 1) {
                            for lz in (z as i32 - 1)..=(z as i32 + 1) {
                                for ly in (surface_y + tree_height - 1)..=(surface_y + tree_height + 1) {
                                    if lx >= 0 && (lx as usize) < CHUNK_SIZE_X && lz >= 0 && (lz as usize) < CHUNK_SIZE_Z && ly < CHUNK_SIZE_Y {
                                        if blocks[lx as usize][lz as usize][ly as usize] == 0 {
                                            blocks[lx as usize][lz as usize][ly as usize] = 18;
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }
        Self { blocks, chunk_x, chunk_z }
    }

    fn serialize_nbt(&self) -> Vec<u8> {
        let mut raw_blocks = vec![0i8; CHUNK_SIZE_X * CHUNK_SIZE_Z * CHUNK_SIZE_Y];
        let mut idx = 0;
        for x in 0..CHUNK_SIZE_X {
            for z in 0..CHUNK_SIZE_Z {
                for y in 0..CHUNK_SIZE_Y {
                    raw_blocks[idx] = self.blocks[x][z][y] as i8;
                    idx += 1;
                }
            }
        }

        let data_array = vec![0i8; (CHUNK_SIZE_X * CHUNK_SIZE_Z * CHUNK_SIZE_Y) / 2];
        let level = ChunkNbtLevel {
            xPos: self.chunk_x,
            zPos: self.chunk_z,
            LastUpdate: 0i64,
            TerrainPopulated: 1,
            Blocks: ByteArray::new(raw_blocks),
            Data: ByteArray::new(data_array.clone()),
            SkyLight: ByteArray::new(vec![-1i8; 16384]),
            BlockLight: ByteArray::new(data_array),
            HeightMap: ByteArray::new(vec![64i8; 256]),
            Entities: vec![],
            TileEntities: vec![],
        };

        let root = ChunkNbtRoot { Level: level };
        fastnbt::to_bytes(&root).unwrap_or_default()
    }

    fn deserialize_nbt(data: &[u8], chunk_x: i32, chunk_z: i32) -> Option<Self> {
        let root: ChunkNbtRoot = fastnbt::from_bytes(data).ok()?;
        let raw_blocks = root.Level.Blocks.as_ref();

        let mut blocks = [[[0; CHUNK_SIZE_Y]; CHUNK_SIZE_Z]; CHUNK_SIZE_X];
        let mut idx = 0;
        for x in 0..CHUNK_SIZE_X {
            for z in 0..CHUNK_SIZE_Z {
                for y in 0..CHUNK_SIZE_Y {
                    if idx < raw_blocks.len() {
                        blocks[x][z][y] = raw_blocks[idx] as u8;
                    }
                    idx += 1;
                }
            }
        }
        Some(Self { blocks, chunk_x, chunk_z })
    }

    fn save_mcr(&self, world_dir: &str) {
        let region_x = self.chunk_x >> 5;
        let region_z = self.chunk_z >> 5;
        let region_path = format!("{}/region/r.{}.{}.mcr", world_dir, region_x, region_z);

        if let Some(parent) = Path::new(&region_path).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let nbt_bytes = self.serialize_nbt();
        let mut encoder = ZlibEncoder::new(Vec::new(), Compression::default());
        if encoder.write_all(&nbt_bytes).is_err() { return; }
        let compressed_data = match encoder.finish() { Ok(d) => d, Err(_) => return };

        let mut file = match File::options().read(true).write(true).create(true).open(&region_path) {
            Ok(f) => f,
            Err(_) => return,
        };

        let file_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        let mut header = vec![0u8; 8192];
        if file_len >= 8192 { let _ = file.read(&mut header); }

        let local_x = (self.chunk_x % 32 + 32) % 32;
        let local_z = (self.chunk_z % 32 + 32) % 32;
        let chunk_index = (local_x + local_z * 32) as usize;

        let length_payload = compressed_data.len() as u32 + 1;
        let sectors_needed = ((length_payload + 4 + 4095) / 4096) as usize;

        let mut max_sector = 2;
        let mut i = 0;
        while i < 1024 {
            let h_pos = i * 4;
            let o = ((header[h_pos] as usize) << 16) | ((header[h_pos + 1] as usize) << 8) | (header[h_pos + 2] as usize);
            let s = header[h_pos + 3] as usize;
            if o + s > max_sector { max_sector = o + s; }
            i += 1;
        }

        let existing_o = ((header[chunk_index * 4] as usize) << 16)
        | ((header[chunk_index * 4 + 1] as usize) << 8)
        | (header[chunk_index * 4 + 2] as usize);
        let existing_s = header[chunk_index * 4 + 3] as usize;

        let sector_offset = if existing_o >= 2 && existing_s >= sectors_needed { existing_o } else { max_sector };

        let header_pos = chunk_index * 4;
        header[header_pos] = ((sector_offset >> 16) & 0xFF) as u8;
        header[header_pos + 1] = ((sector_offset >> 8) & 0xFF) as u8;
        header[header_pos + 2] = (sector_offset & 0xFF) as u8;
        header[header_pos + 3] = sectors_needed as u8;

        let timestamp_pos = 4096 + chunk_index * 4;
        let current_time = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap().as_secs() as u32;
        header[timestamp_pos..timestamp_pos + 4].copy_from_slice(&current_time.to_be_bytes());

        let _ = file.seek(SeekFrom::Start(0));
        let _ = file.write_all(&header);

        let _ = file.seek(SeekFrom::Start((sector_offset * 4096) as u64));
        let _ = file.write_all(&(length_payload.to_be_bytes()));
        let _ = file.write_all(&[2u8]);
        let _ = file.write_all(&compressed_data);

        let total_sectors = sector_offset + sectors_needed;
        let current_len = file.metadata().map(|m| m.len()).unwrap_or(0);
        let new_len = (total_sectors * 4096) as u64;
        if new_len > current_len { let _ = file.set_len(new_len); }
    }

    fn load_mcr(world_dir: &str, chunk_x: i32, chunk_z: i32) -> Option<Self> {
        let region_x = chunk_x >> 5;
        let region_z = chunk_z >> 5;
        let region_path = format!("{}/region/r.{}.{}.mcr", world_dir, region_x, region_z);

        if !Path::new(&region_path).exists() { return None; }

        let mut file = File::open(&region_path).ok()?;
        let mut header = vec![0u8; 8192];
        file.read_exact(&mut header).ok()?;

        let local_x = (chunk_x % 32 + 32) % 32;
        let local_z = (chunk_z % 32 + 32) % 32;
        let chunk_index = (local_x + local_z * 32) as usize;

        let header_pos = chunk_index * 4;
        let sector_offset = ((header[header_pos] as usize) << 16) | ((header[header_pos + 1] as usize) << 8) | header[header_pos + 2] as usize;
        if sector_offset == 0 { return None; }

        file.seek(SeekFrom::Start((sector_offset * 4096) as u64)).ok()?;

        let mut length_bytes = [0u8; 4];
        file.read_exact(&mut length_bytes).ok()?;
        let length = u32::from_be_bytes(length_bytes) as usize;
        if length < 2 { return None; }

        let mut compression_type = [0u8; 1];
        file.read_exact(&mut compression_type).ok()?;

        let mut compressed_data = vec![0u8; length - 1];
        file.read_exact(&mut compressed_data).ok()?;

        if compression_type[0] != 2 { return None; }

        let mut decoder = ZlibDecoder::new(&compressed_data[..]);
        let mut nbt_bytes = Vec::new();
        decoder.read_to_end(&mut nbt_bytes).ok()?;

        Self::deserialize_nbt(&nbt_bytes, chunk_x, chunk_z)
    }

    fn get_block_global(chunks: &HashMap<(i32, i32), Chunk>, wx: i32, wy: i32, wz: i32) -> Block {
        if wy < 0 || wy >= CHUNK_SIZE_Y as i32 { return 0; }
        let cx = wx.div_euclid(16);
        let cz = wz.div_euclid(16);
        let lx = wx.rem_euclid(16) as usize;
        let lz = wz.rem_euclid(16) as usize;
        let ly = wy as usize;

        if let Some(chunk) = chunks.get(&(cx, cz)) {
            chunk.blocks[lx][lz][ly]
        } else {
            0
        }
    }

    fn set_block_global(chunks: &mut HashMap<(i32, i32), Chunk>, wx: i32, wy: i32, wz: i32, block: Block) {
        if wy < 0 || wy >= CHUNK_SIZE_Y as i32 { return; }
        let cx = wx.div_euclid(16);
        let cz = wz.div_euclid(16);
        let lx = wx.rem_euclid(16) as usize;
        let lz = wz.rem_euclid(16) as usize;
        let ly = wy as usize;

        if let Some(chunk) = chunks.get_mut(&(cx, cz)) {
            chunk.blocks[lx][lz][ly] = block;
        }
    }

    fn get_block_tex_coords(block: Block, face: usize) -> i32 {
        match block {
            1 => 1,
            2 => match face { 2 => 0, 3 => 2, _ => 3 },
            3 => 2,
            4 => 16,
            5 => 4,
            7 => 17,
            8 | 9 => 205,
            12 => 18,
            13 => 19,
            14 => 32,
            15 => 33,
            16 => 34,
            17 => match face { 2 | 3 => 21, _ => 20 },
            18 => 52,
            20 => 49,
            35 => 64,
            41 => 23,
            42 => 22,
            45 => 7,
            46 => match face { 2 => 8, 3 => 9, _ => 10 },
            47 => 35,
            48 => 36,
            49 => 37,
            54 => match face { 2 => 26, 3 => 26, 4 | 5 => 25, _ => 24 },
            56 => 50,
            57 => 24,
            61 | 62 => match face { 2 => 44, 3 => 44, 4 => 45, 5 => 45, _ => 44 },
            _ => 1,
        }
    }

    fn generate_mesh(&self, chunks: &HashMap<(i32, i32), Chunk>, camera_in_water: bool) -> Vec<Vertex> {
        let mut vertices = Vec::new();
        for x in 0..CHUNK_SIZE_X {
            for y in 0..CHUNK_SIZE_Y {
                for z in 0..CHUNK_SIZE_Z {
                    let block = self.blocks[x][z][y];
                    if block == 0 { continue; }

                    if block == 9 && camera_in_water { continue; }

                    let wx = self.chunk_x * 16 + x as i32;
                    let wy = y as i32;
                    let wz = self.chunk_z * 16 + z as i32;

                    let fx = wx as f32;
                    let fy = wy as f32;
                    let fz = wz as f32;

                    let neighbor_z_plus = Self::get_block_global(chunks, wx, wy, wz + 1);
                    let neighbor_z_minus = Self::get_block_global(chunks, wx, wy, wz - 1);
                    let neighbor_y_plus = Self::get_block_global(chunks, wx, wy + 1, wz);
                    let neighbor_y_minus = Self::get_block_global(chunks, wx, wy - 1, wz);
                    let neighbor_x_plus = Self::get_block_global(chunks, wx + 1, wy, wz);
                    let neighbor_x_minus = Self::get_block_global(chunks, wx - 1, wy, wz);

                    let should_render_face = |neighbor: Block, current: Block| -> bool {
                        if neighbor == 0 { return true; }
                        if current == 9 && neighbor == 9 { return false; }
                        if current == 9 && neighbor != 9 { return true; }
                        if current != 9 && neighbor == 9 { return false; }
                        if (current == 18 || current == 20) && neighbor == current { return false; }
                        false
                    };

                    if neighbor_z_plus == 0 || should_render_face(neighbor_z_plus, block) {
                        let tx = Self::get_block_tex_coords(block, 0);
                        add_textured_face(&mut vertices, [[fx - 0.5, fy - 0.5, fz + 0.5], [fx + 0.5, fy - 0.5, fz + 0.5], [fx + 0.5, fy + 0.5, fz + 0.5], [fx + 0.5, fy + 0.5, fz + 0.5], [fx - 0.5, fy + 0.5, fz + 0.5], [fx - 0.5, fy - 0.5, fz + 0.5]], tx);
                    }
                    if neighbor_z_minus == 0 || should_render_face(neighbor_z_minus, block) {
                        let tx = Self::get_block_tex_coords(block, 1);
                        add_textured_face(&mut vertices, [[fx + 0.5, fy - 0.5, fz - 0.5], [fx - 0.5, fy - 0.5, fz - 0.5], [fx - 0.5, fy + 0.5, fz - 0.5], [fx - 0.5, fy + 0.5, fz - 0.5], [fx + 0.5, fy + 0.5, fz - 0.5], [fx + 0.5, fy - 0.5, fz - 0.5]], tx);
                    }
                    if neighbor_y_plus == 0 || should_render_face(neighbor_y_plus, block) {
                        let tx = Self::get_block_tex_coords(block, 2);
                        add_textured_face(&mut vertices, [[fx - 0.5, fy + 0.5, fz + 0.5], [fx + 0.5, fy + 0.5, fz + 0.5], [fx + 0.5, fy + 0.5, fz - 0.5], [fx + 0.5, fy + 0.5, fz - 0.5], [fx - 0.5, fy + 0.5, fz - 0.5], [fx - 0.5, fy + 0.5, fz + 0.5]], tx);
                    }
                    if neighbor_y_minus == 0 || should_render_face(neighbor_y_minus, block) {
                        let tx = Self::get_block_tex_coords(block, 3);
                        add_textured_face(&mut vertices, [[fx - 0.5, fy - 0.5, fz - 0.5], [fx + 0.5, fy - 0.5, fz - 0.5], [fx + 0.5, fy - 0.5, fz + 0.5], [fx + 0.5, fy - 0.5, fz + 0.5], [fx - 0.5, fy - 0.5, fz + 0.5], [fx - 0.5, fy - 0.5, fz - 0.5]], tx);
                    }
                    if neighbor_x_plus == 0 || should_render_face(neighbor_x_plus, block) {
                        let tx = Self::get_block_tex_coords(block, 4);
                        add_textured_face(&mut vertices, [[fx + 0.5, fy - 0.5, fz + 0.5], [fx + 0.5, fy - 0.5, fz - 0.5], [fx + 0.5, fy + 0.5, fz - 0.5], [fx + 0.5, fy + 0.5, fz - 0.5], [fx + 0.5, fy + 0.5, fz + 0.5], [fx + 0.5, fy - 0.5, fz + 0.5]], tx);
                    }
                    if neighbor_x_minus == 0 || should_render_face(neighbor_x_minus, block) {
                        let tx = Self::get_block_tex_coords(block, 5);
                        add_textured_face(&mut vertices, [[fx - 0.5, fy - 0.5, fz - 0.5], [fx - 0.5, fy - 0.5, fz + 0.5], [fx - 0.5, fy + 0.5, fz + 0.5], [fx - 0.5, fy + 0.5, fz + 0.5], [fx - 0.5, fy + 0.5, fz - 0.5], [fx - 0.5, fy - 0.5, fz - 0.5]], tx);
                    }
                }
            }
        }
        vertices
    }
}

fn prepare_world_structure(world_dir: &str) {
    let _ = std::fs::create_dir_all(format!("{}/region", world_dir));
    let _ = std::fs::create_dir_all(format!("{}/data", world_dir));

    let lock_path = format!("{}/session.lock", world_dir);
    if !Path::new(&lock_path).exists() {
        let _ = std::fs::write(&lock_path, &0i64.to_be_bytes());
    }

    let level_path = format!("{}/level.dat", world_dir);
    if !Path::new(&level_path).exists() {
        let mut nbt = Vec::new();
        nbt.push(0x0A);
        nbt.write_all(&0u16.to_be_bytes()).unwrap();

        nbt.push(0x0A);
        nbt.write_all(&4u16.to_be_bytes()).unwrap();
        nbt.write_all(b"Data").unwrap();

        nbt.push(0x03);
        nbt.write_all(&7u16.to_be_bytes()).unwrap();
        nbt.write_all(b"version").unwrap();
        nbt.write_all(&19132i32.to_be_bytes()).unwrap();

        nbt.push(0x08);
        nbt.write_all(&9u16.to_be_bytes()).unwrap();
        nbt.write_all(b"LevelName").unwrap();
        let world_name_bytes = b"OpenMCWorld";
        nbt.write_all(&(world_name_bytes.len() as u16).to_be_bytes()).unwrap();
        nbt.write_all(world_name_bytes).unwrap();

        nbt.push(0x03);
        nbt.write_all(&6u16.to_be_bytes()).unwrap();
        nbt.write_all(b"SpawnX").unwrap();
        nbt.write_all(&8i32.to_be_bytes()).unwrap();

        nbt.push(0x03);
        nbt.write_all(&6u16.to_be_bytes()).unwrap();
        nbt.write_all(b"SpawnY").unwrap();
        nbt.write_all(&68i32.to_be_bytes()).unwrap();

        nbt.push(0x03);
        nbt.write_all(&6u16.to_be_bytes()).unwrap();
        nbt.write_all(b"SpawnZ").unwrap();
        nbt.write_all(&8i32.to_be_bytes()).unwrap();

        nbt.push(0x00);
        nbt.push(0x00);

        let mut encoder = GzEncoder::new(Vec::new(), Compression::default());
        if encoder.write_all(&nbt).is_ok() {
            if let Ok(compressed) = encoder.finish() {
                let _ = std::fs::write(&level_path, compressed);
            }
        }
    }
}

fn add_textured_face(vertices: &mut Vec<Vertex>, positions: [[f32; 3]; 6], tex_index: i32) {
    let u_slot = (tex_index % 16) as f32;
    let v_slot = (15 - (tex_index / 16)) as f32;
    let du = 1.0 / 16.0;
    let dv = 1.0 / 16.0;
    let u0 = u_slot * du;
    let v0 = v_slot * dv;
    let u1 = u0 + du;
    let v1 = v0 + dv;

    let uvs = [[u0, v0], [u1, v0], [u1, v1], [u1, v1], [u0, v1], [u0, v0]];
    for i in 0..6 { vertices.push(Vertex { position: positions[i], uv: uvs[i] }); }
}

struct ItemEntity {
    pos: Vec3,
    velocity: Vec3,
    block_id: Block,
    rotation: f32,
}

impl ItemEntity {
    fn new(pos: Vec3, block_id: Block) -> Self {
        Self {
            pos,
            velocity: Vec3::new((rand_pseudo() - 0.5) * 0.1, 0.15, (rand_pseudo() - 0.5) * 0.1),
            block_id,
            rotation: 0.0,
        }
    }

    fn update(&mut self, chunks: &HashMap<(i32, i32), Chunk>, player_pos: Vec3) -> bool {
        self.rotation += 2.0;
        self.velocity.y -= 0.01;
        if self.velocity.y < -0.4 { self.velocity.y = -0.4; }

        self.pos += self.velocity;
        let bx = self.pos.x.floor() as i32;
        let by = (self.pos.y - 0.25).floor() as i32;
        let bz = self.pos.z.floor() as i32;

        if Chunk::get_block_global(chunks, bx, by, bz) != 0 {
            self.pos.y = by as f32 + 0.75;
            self.velocity.y = 0.0;
            self.velocity.x *= 0.8;
            self.velocity.z *= 0.8;
        }

        let dist_sq = (self.pos - player_pos).length_squared();
        dist_sq < 1.5
    }
}

fn rand_pseudo() -> f32 {
    use std::time::{SystemTime, UNIX_EPOCH};
    let nanos = SystemTime::now().duration_since(UNIX_EPOCH).unwrap().subsec_nanos();
    ((nanos % 1000) as f32) / 1000.0
}

fn get_wireframe_vertices(bx: i32, by: i32, bz: i32) -> Vec<Vertex> {
    let fx = bx as f32; let fy = by as f32; let fz = bz as f32;
    let min_x = fx - 0.502; let max_x = fx + 0.502;
    let min_y = fy - 0.502; let max_y = fy + 0.502;
    let min_z = fz - 0.502; let max_z = fz + 0.502;

    let corners = [
        [min_x, min_y, min_z], [max_x, min_y, min_z],
        [max_x, min_y, max_z], [min_x, min_y, max_z],
        [min_x, max_y, min_z], [max_x, max_y, min_z],
        [max_x, max_y, max_z], [min_x, max_y, max_z],
    ];
    let edges = [(0,1), (1,2), (2,3), (3,0), (4,5), (5,6), (6,7), (7,4), (0,4), (1,5), (2,6), (3,7)];
    let mut verts = Vec::new();
    for (s, e) in edges {
        verts.push(Vertex { position: corners[s], uv: [0.0, 0.0] });
        verts.push(Vertex { position: corners[e], uv: [0.0, 0.0] });
    }
    verts
}

fn get_colliding_blocks(chunks: &HashMap<(i32, i32), Chunk>, pos: Vec3, radius: f32, height: f32) -> Vec<(f32, f32, f32, f32, f32, f32)> {
    let mut boxes = Vec::new();
    let min_x = (pos.x - radius - 0.5).floor() as i32;
    let max_x = (pos.x + radius + 0.5).floor() as i32;
    let min_y = (pos.y - 0.5).floor() as i32;
    let max_y = (pos.y + height + 0.5).floor() as i32;
    let min_z = (pos.z - radius - 0.5).floor() as i32;
    let max_z = (pos.z + radius + 0.5).floor() as i32;

    for x in min_x..=max_x {
        for y in min_y..=max_y {
            for z in min_z..=max_z {
                let block = Chunk::get_block_global(chunks, x, y, z);
                if block != 0 && block != 9 {
                    boxes.push((x as f32 - 0.5, x as f32 + 0.5, y as f32 - 0.5, y as f32 + 0.5, z as f32 - 0.5, z as f32 + 0.5));
                }
            }
        }
    }
    boxes
}

fn check_block_intersects_player(bx: i32, by: i32, bz: i32, player_pos: Vec3, radius: f32, height: f32) -> bool {
    let b_min_x = bx as f32 - 0.5; let b_max_x = bx as f32 + 0.5;
    let b_min_y = by as f32 - 0.5; let b_max_y = by as f32 + 0.5;
    let b_min_z = bz as f32 - 0.5; let b_max_z = bz as f32 + 0.5;

    let p_min_x = player_pos.x - radius; let p_max_x = player_pos.x + radius;
    let p_min_y = player_pos.y; let p_max_y = player_pos.y + height;
    let p_min_z = player_pos.z - radius; let p_max_z = player_pos.z + radius;

    p_max_x > b_min_x && p_min_x < b_max_x && p_max_y > b_min_y && p_min_y < b_max_y && p_max_z > b_min_z && p_min_z < b_max_z
}

fn raycast(chunks: &HashMap<(i32, i32), Chunk>, origin: Vec3, direction: Vec3, max_dist: f32) -> Option<(i32, i32, i32, i32, i32, i32)> {
    let mut t = 0.0;
    let step = 0.05;
    let mut last_air = None;

    while t < max_dist {
        let p = origin + direction * t;
        let bx = (p.x + 0.5).floor() as i32;
        let by = (p.y + 0.5).floor() as i32;
        let bz = (p.z + 0.5).floor() as i32;

        let block = Chunk::get_block_global(chunks, bx, by, bz);
        if block != 0 && block != 9 {
            if let Some((lax, lay, laz)) = last_air { return Some((bx, by, bz, lax, lay, laz)); }
            return None;
        } else {
            last_air = Some((bx, by, bz));
        }
        t += step;
    }
    None
}

fn generate_item_mesh(block_id: Block) -> Vec<Vertex> {
    let mut vertices = Vec::new();
    let s = 0.25;
    let tx = Chunk::get_block_tex_coords(block_id, 2);
    let faces = [
        [[-s, -s, s], [s, -s, s], [s, s, s], [s, s, s], [-s, s, s], [-s, -s, s]],
        [[s, -s, -s], [-s, -s, -s], [-s, s, -s], [-s, s, -s], [s, s, -s], [s, -s, -s]],
        [[-s, s, -s], [s, s, -s], [s, s, s], [s, s, s], [-s, s, s], [-s, s, -s]],
        [[-s, -s, s], [s, -s, s], [s, -s, -s], [s, -s, -s], [-s, -s, -s], [-s, -s, s]],
        [[s, -s, s], [s, -s, -s], [s, s, -s], [s, s, -s], [s, s, s], [s, -s, s]],
        [[-s, -s, -s], [-s, -s, s], [-s, s, s], [-s, s, s], [-s, s, -s], [-s, -s, -s]],
    ];
    for face in faces {
        let mut face_verts = Vec::new();
        add_textured_face(&mut face_verts, face, tx);
        vertices.extend(face_verts);
    }
    vertices
}

fn main() {
    extract_assets_from_jar();

    let event_loop = EventLoop::new().unwrap();
    let window_builder = WindowBuilder::new()
    .with_title("OpenMC")
    .with_inner_size(winit::dpi::LogicalSize::new(800, 600));

    let template = ConfigTemplateBuilder::new();
    let display_builder = DisplayBuilder::new().with_window_builder(Some(window_builder));

    let (window, gl_config) = display_builder
    .build(&event_loop, template, |configs| {
        configs.reduce(|a, b| if a.num_samples() > b.num_samples() { a } else { b }).unwrap()
    })
    .unwrap();

    let window = window.unwrap();
    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
    window.set_cursor_visible(true);

    let raw_window_handle = window.raw_window_handle();
    let gl_display = gl_config.display();
    let context_attributes = ContextAttributesBuilder::new().build(Some(raw_window_handle));
    let gl_context = unsafe { gl_display.create_context(&gl_config, &context_attributes).expect("GL context failed") };

    let size = window.inner_size();
    let width = NonZeroU32::new(size.width.max(1)).unwrap();
    let height = NonZeroU32::new(size.height.max(1)).unwrap();
    let surface_attrs = glutin::surface::SurfaceAttributesBuilder::<WindowSurface>::new().build(raw_window_handle, width, height);
    let gl_surface = unsafe { gl_display.create_window_surface(&gl_config, &surface_attrs).unwrap() };
    let gl_context = gl_context.make_current(&gl_surface).unwrap();

    let gl = unsafe { glow::Context::from_loader_function(|s| gl_display.get_proc_address(&std::ffi::CString::new(s).unwrap())) };

    let sound_manager = SoundManager::new();

    unsafe {
        gl.enable(glow::DEPTH_TEST);
        gl.enable(glow::CULL_FACE);
        gl.cull_face(glow::BACK);
        gl.enable(glow::BLEND);
        gl.blend_func(glow::SRC_ALPHA, glow::ONE_MINUS_SRC_ALPHA);
        gl.line_width(2.0);
    }

    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("src/terrain.png");
    let img = image::open(&path).expect("Brak terrain.png w folderze src!").flipv();
    let img_data = img.to_rgba8();
    let (tex_width, tex_height) = img.dimensions();

    let texture = unsafe {
        let tex = gl.create_texture().unwrap();
        gl.bind_texture(glow::TEXTURE_2D, Some(tex));
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MIN_FILTER, glow::NEAREST as i32);
        gl.tex_parameter_i32(glow::TEXTURE_2D, glow::TEXTURE_MAG_FILTER, glow::NEAREST as i32);
        gl.tex_image_2d(glow::TEXTURE_2D, 0, glow::RGBA as i32, tex_width as i32, tex_height as i32, 0, glow::RGBA, glow::UNSIGNED_BYTE, Some(&img_data));
        tex
    };

    let world_dir = "saves/OpenMCWorld";
    prepare_world_structure(world_dir);

    let (initial_pos, initial_yaw, initial_pitch) = load_player_spawn_from_level(world_dir);
    let mut player_pos = initial_pos;
    let mut yaw = initial_yaw;
    let mut pitch = initial_pitch;

    let mut chunks: HashMap<(i32, i32), Chunk> = HashMap::new();
    let mut gpu_chunks: HashMap<(i32, i32), GpuChunk> = HashMap::new();
    let render_distance = 3;

    let hotbar_blocks: [Block; 9] = [1, 2, 3, 4, 12, 13, 20, 35, 45];
    let mut selected_slot: usize = 0;

    let mut menu = menu::MainMenuManager::new(&gl);

    let (chunk_tx, chunk_rx): (Sender<(i32, i32, Chunk, Vec<Vertex>)>, Receiver<(i32, i32, Chunk, Vec<Vertex>)>) = channel();

    for cx in -render_distance..=render_distance {
        for cz in -render_distance..=render_distance {
            let world_dir_clone = world_dir.to_string();
            let tx = chunk_tx.clone();
            thread::spawn(move || {
                let chunk = Chunk::load_mcr(&world_dir_clone, cx, cz).unwrap_or_else(|| {
                    let new_chunk = Chunk::new_generated(cx, cz);
                    new_chunk.save_mcr(&world_dir_clone);
                    new_chunk
                });
                let mut dummy_map = HashMap::new();
                dummy_map.insert((cx, cz), chunk.clone());
                let mesh = chunk.generate_mesh(&dummy_map, false);
                let _ = tx.send((cx, cz, chunk, mesh));
            });
        }
    }

    let mut entities: Vec<ItemEntity> = Vec::new();

    // Inicjalizujemy moby korzystając z pełnego modułu mobs.rs
    let mut mobs: Vec<Mob> = vec![
        Mob::new(player_pos + Vec3::new(5.0, 0.0, 5.0), MobType::Zombie),
        Mob::new(player_pos + Vec3::new(-5.0, 0.0, 5.0), MobType::Creeper),
        Mob::new(player_pos + Vec3::new(3.0, 0.0, -5.0), MobType::Pig),
    ];

    let shader_program = unsafe {
        let program = gl.create_program().unwrap();
        let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
        gl.shader_source(vs, "#version 330 core
        layout (location = 0) in vec3 aPos;
        layout (location = 1) in vec2 aTexCoord;
        out vec2 TexCoord;
        uniform mat4 uMVP;
        void main() {
        gl_Position = uMVP * vec4(aPos, 1.0);
        TexCoord = aTexCoord;
    }");
        gl.compile_shader(vs);
        let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
        gl.shader_source(fs, "#version 330 core
        in vec2 TexCoord;
        out vec4 FragColor;
        uniform sampler2D uTexture;
        void main() {
        vec4 texColor = texture(uTexture, TexCoord);
        if(texColor.a < 0.1) discard;
        FragColor = texColor;
    }");
        gl.compile_shader(fs);
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
        program
    };

    let crosshair_shader = unsafe {
        let program = gl.create_program().unwrap();
        let vs = gl.create_shader(glow::VERTEX_SHADER).unwrap();
        gl.shader_source(vs, "#version 330 core
        layout (location = 0) in vec2 aPos;
        void main() { gl_Position = vec4(aPos, 0.0, 1.0); }");
        gl.compile_shader(vs);
        let fs = gl.create_shader(glow::FRAGMENT_SHADER).unwrap();
        gl.shader_source(fs, "#version 330 core
        out vec4 FragColor;
        void main() { FragColor = vec4(1.0, 1.0, 1.0, 0.8); }");
        gl.compile_shader(fs);
        gl.attach_shader(program, vs);
        gl.attach_shader(program, fs);
        gl.link_program(program);
        program
    };

    let (item_vao, item_vbo) = unsafe {
        let vao = gl.create_vertex_array().unwrap();
        gl.bind_vertex_array(Some(vao));
        let vbo = gl.create_buffer().unwrap();
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let stride = std::mem::size_of::<Vertex>() as i32;
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 12);
        gl.enable_vertex_attrib_array(1);
        gl.bind_vertex_array(None);
        (vao, vbo)
    };

    let (outline_vao, outline_vbo) = unsafe {
        let vao = gl.create_vertex_array().unwrap();
        gl.bind_vertex_array(Some(vao));
        let vbo = gl.create_buffer().unwrap();
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        gl.buffer_data_size(glow::ARRAY_BUFFER, (24 * std::mem::size_of::<Vertex>()) as i32, glow::DYNAMIC_DRAW);
        let stride = std::mem::size_of::<Vertex>() as i32;
        gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
        gl.enable_vertex_attrib_array(0);
        gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 12);
        gl.enable_vertex_attrib_array(1);
        gl.bind_vertex_array(None);
        (vao, vbo)
    };

    let crosshair_vao = unsafe {
        let vao = gl.create_vertex_array().unwrap();
        gl.bind_vertex_array(Some(vao));
        let vbo = gl.create_buffer().unwrap();
        gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
        let size = 0.025;
        let aspect = 800.0 / 600.0;
        let crosshair_verts: [f32; 8] = [-size / aspect, 0.0, size / aspect, 0.0, 0.0, -size, 0.0, size];
        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&crosshair_verts), glow::STATIC_DRAW);
        gl.vertex_attrib_pointer_f32(0, 2, glow::FLOAT, false, 8, 0);
        gl.enable_vertex_attrib_array(0);
        gl.bind_vertex_array(None);
        vao
    };

    let mut velocity = Vec3::ZERO;
    let player_radius = 0.3;
    let player_height = 1.8;
    let eye_height = 1.62;

    let mut camera_front = Vec3::new(
        yaw.to_radians().cos() * pitch.to_radians().cos(),
                                     pitch.to_radians().sin(),
                                     yaw.to_radians().sin() * pitch.to_radians().cos(),
    ).normalize();
    let camera_up = Vec3::Y;

    let mut pressed_keys = HashSet::new();
    let mut requested_chunks = HashSet::new();
    let mut last_in_water = false;
    let mut step_timer = 0.0_f32;

    event_loop.run(move |event, target| {
        match event {
            Event::WindowEvent { event, .. } => match event {
                WindowEvent::CloseRequested => {
                    if menu.state == GameState::Playing {
                        save_player_data_to_level(world_dir, player_pos, yaw, pitch);
                        for chunk in chunks.values() { chunk.save_mcr(world_dir); }
                    }
                    target.exit();
                }
                WindowEvent::Resized(size) => {
                    if size.width > 0 && size.height > 0 {
                        gl_surface.resize(&gl_context, NonZeroU32::new(size.width).unwrap(), NonZeroU32::new(size.height).unwrap());
                        unsafe { gl.viewport(0, 0, size.width as i32, size.height as i32); }
                    }
                }
                WindowEvent::KeyboardInput { event: KeyEvent { physical_key, state: ElementState::Pressed, .. }, .. } => {
                    if let PhysicalKey::Code(code) = physical_key {
                        match code {
                            KeyCode::Escape => {
                                if menu.state == GameState::Playing {
                                    save_player_data_to_level(world_dir, player_pos, yaw, pitch);
                                    for chunk in chunks.values() { chunk.save_mcr(world_dir); }
                                    menu.show_main_menu();
                                    let _ = window.set_cursor_grab(winit::window::CursorGrabMode::None);
                                    window.set_cursor_visible(true);
                                } else if menu.state == GameState::Options {
                                    menu.back();
                                } else {
                                    target.exit();
                                }
                            }
                            KeyCode::Digit1 if menu.state == GameState::Playing => selected_slot = 0,
                            KeyCode::Digit2 if menu.state == GameState::Playing => selected_slot = 1,
                            KeyCode::Digit3 if menu.state == GameState::Playing => selected_slot = 2,
                            KeyCode::Digit4 if menu.state == GameState::Playing => selected_slot = 3,
                            KeyCode::Digit5 if menu.state == GameState::Playing => selected_slot = 4,
                            KeyCode::Digit6 if menu.state == GameState::Playing => selected_slot = 5,
                            KeyCode::Digit7 if menu.state == GameState::Playing => selected_slot = 6,
                            KeyCode::Digit8 if menu.state == GameState::Playing => selected_slot = 7,
                            KeyCode::Digit9 if menu.state == GameState::Playing => selected_slot = 8,
                            _ => {}
                        }
                        pressed_keys.insert(code);
                    }
                }
                WindowEvent::KeyboardInput { event: KeyEvent { physical_key, state: ElementState::Released, .. }, .. } => {
                    if let PhysicalKey::Code(code) = physical_key {
                        pressed_keys.remove(&code);
                    }
                }
                WindowEvent::MouseWheel { delta, .. } => {
                    if menu.state == GameState::Playing {
                        match delta {
                            MouseScrollDelta::LineDelta(_, y) => {
                                if y > 0.0 { selected_slot = (selected_slot + 8) % 9; }
                                else if y < 0.0 { selected_slot = (selected_slot + 1) % 9; }
                            }
                            MouseScrollDelta::PixelDelta(pos) => {
                                if pos.y > 0.0 { selected_slot = (selected_slot + 8) % 9; }
                                else if pos.y < 0.0 { selected_slot = (selected_slot + 1) % 9; }
                            }
                        }
                    }
                }
                WindowEvent::MouseInput { state: ElementState::Pressed, button, .. } => {
                    if menu.state == GameState::MainMenu || menu.state == GameState::Options {
                        let action = menu.click();
                        match action {
                            menu::MenuAction::Play => {
                                let _ = window.set_cursor_grab(winit::window::CursorGrabMode::Locked);
                                window.set_cursor_visible(false);
                            }
                            menu::MenuAction::Quit => {
                                target.exit();
                            }
                            menu::MenuAction::None => {}
                        }
                    } else if menu.state == GameState::Playing && button == MouseButton::Left {
                        let eye_pos = player_pos + Vec3::new(0.0, eye_height, 0.0);
                        if let Some((bx, by, bz, lax, lay, laz)) = raycast(&chunks, eye_pos, camera_front, 5.0) {
                            let target_cx = bx.div_euclid(16);
                            let target_cz = bz.div_euclid(16);

                            let broken_block = Chunk::get_block_global(&chunks, bx, by, bz);
                            Chunk::set_block_global(&mut chunks, bx, by, bz, 0);

                            if let Some(ref sm) = sound_manager {
                                sm.play_dig(broken_block);
                            }

                            if broken_block != 0 {
                                entities.push(ItemEntity::new(Vec3::new(bx as f32, by as f32, bz as f32), broken_block));
                            }

                            if let Some(chunk) = chunks.get(&(target_cx, target_cz)) {
                                let chunk_clone = chunk.clone();
                                let chunks_clone = chunks.clone();
                                let tx = chunk_tx.clone();
                                let head_block = Chunk::get_block_global(&chunks, player_pos.x.floor() as i32, (player_pos.y + eye_height).floor() as i32, player_pos.z.floor() as i32);
                                let in_water = head_block == 9;
                                thread::spawn(move || {
                                    let mesh = chunk_clone.generate_mesh(&chunks_clone, in_water);
                                    let _ = tx.send((target_cx, target_cz, chunk_clone, mesh));
                                });
                            }
                        }
                    } else if menu.state == GameState::Playing && button == MouseButton::Right {
                        let eye_pos = player_pos + Vec3::new(0.0, eye_height, 0.0);
                        if let Some((_, _, _, lax, lay, laz)) = raycast(&chunks, eye_pos, camera_front, 5.0) {
                            let target_cx = lax.div_euclid(16);
                            let target_cz = laz.div_euclid(16);

                            if !check_block_intersects_player(lax, lay, laz, player_pos, player_radius, player_height) {
                                let active_block = hotbar_blocks[selected_slot];
                                Chunk::set_block_global(&mut chunks, lax, lay, laz, active_block);

                                if let Some(ref sm) = sound_manager {
                                    sm.play_dig(active_block);
                                }

                                if let Some(chunk) = chunks.get(&(target_cx, target_cz)) {
                                    let chunk_clone = chunk.clone();
                                    let chunks_clone = chunks.clone();
                                    let tx = chunk_tx.clone();
                                    let head_block = Chunk::get_block_global(&chunks, player_pos.x.floor() as i32, (player_pos.y + eye_height).floor() as i32, player_pos.z.floor() as i32);
                                    let in_water = head_block == 9;
                                    thread::spawn(move || {
                                        let mesh = chunk_clone.generate_mesh(&chunks_clone, in_water);
                                        let _ = tx.send((target_cx, target_cz, chunk_clone, mesh));
                                    });
                                }
                            }
                        }
                    }
                }
                WindowEvent::CursorMoved { position, .. } => {
                    let size = window.inner_size();
                    menu.update_cursor(position.x, position.y, size.width, size.height);

                    if menu.state == GameState::Playing {
                        let center_x = size.width as f64 / 2.0;
                        let center_y = size.height as f64 / 2.0;
                        let dx = (position.x - center_x) as f32;
                        let dy = (position.y - center_y) as f32;

                        if dx != 0.0 || dy != 0.0 {
                            let sensitivity = 0.15;
                            yaw += dx * sensitivity;
                            pitch -= dy * sensitivity;
                            pitch = pitch.clamp(-89.0, 89.0);

                            let front = Vec3::new(
                                yaw.to_radians().cos() * pitch.to_radians().cos(),
                                                  pitch.to_radians().sin(),
                                                  yaw.to_radians().sin() * pitch.to_radians().cos(),
                            );
                            camera_front = front.normalize();
                            let _ = window.set_cursor_position(winit::dpi::PhysicalPosition::new(center_x, center_y));
                        }
                    }
                }
                WindowEvent::RedrawRequested => {
                    unsafe {
                        gl.clear_color(0.2, 0.3, 0.4, 1.0);
                        gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);
                    }

                    if menu.state == GameState::MainMenu || menu.state == GameState::Options {
                        let size = window.inner_size();
                        menu.render(&gl, texture, size.width, size.height);
                    } else {
                        let head_block = Chunk::get_block_global(&chunks, player_pos.x.floor() as i32, (player_pos.y + eye_height).floor() as i32, player_pos.z.floor() as i32);
                        let in_water = head_block == 9;

                        if in_water != last_last_in_water_state(&mut last_in_water, in_water) {
                            requested_chunks.clear();
                        }

                        let move_speed = if in_water { 0.04 } else { 0.08 };
                        let mut move_dir = Vec3::ZERO;
                        let flat_front = Vec3::new(camera_front.x, 0.0, camera_front.z).normalize_or_zero();
                        let flat_right = flat_front.cross(Vec3::Y).normalize_or_zero();

                        if pressed_keys.contains(&KeyCode::KeyW) { move_dir += flat_front; }
                        if pressed_keys.contains(&KeyCode::KeyS) { move_dir -= flat_front; }
                        if pressed_keys.contains(&KeyCode::KeyA) { move_dir -= flat_right; }
                        if pressed_keys.contains(&KeyCode::KeyD) { move_dir += flat_right; }

                        let mut computed_velocity_x = velocity.x;
                        let mut computed_velocity_z = velocity.z;

                        if move_dir.length_squared() > 0.0 {
                            move_dir = move_dir.normalize() * move_speed;
                            step_timer += move_speed;
                            if step_timer > 0.6 {
                                step_timer = 0.0;
                                let ground_block = Chunk::get_block_global(&chunks, player_pos.x.floor() as i32, (player_pos.y - 0.1).floor() as i32, player_pos.z.floor() as i32);
                                if let Some(ref sm) = sound_manager {
                                    sm.play_step(ground_block);
                                }
                            }
                        }

                        computed_velocity_x = move_dir.x;
                        computed_velocity_z = move_dir.z;

                        velocity.x = computed_velocity_x;
                        velocity.z = computed_velocity_z;

                        if in_water {
                            velocity.y -= 0.005;
                            if velocity.y < -0.15 { velocity.y = -0.15; }
                            if pressed_keys.contains(&KeyCode::Space) {
                                velocity.y = 0.08;
                            }
                        } else {
                            velocity.y -= 0.012;
                            if velocity.y < -0.5 { velocity.y = -0.5; }
                        }

                        player_pos.x += velocity.x;
                        for (b_min_x, b_max_x, b_min_y, b_max_y, b_min_z, b_max_z) in get_colliding_blocks(&chunks, player_pos, player_radius, player_height) {
                            if player_pos.x + player_radius > b_min_x && player_pos.x - player_radius < b_max_x &&
                                player_pos.y + player_height > b_min_y && player_pos.y < b_max_y &&
                                player_pos.z + player_radius > b_min_z && player_pos.z - player_radius < b_max_z {
                                    if velocity.x > 0.0 { player_pos.x = b_min_x - player_radius; }
                                    else if velocity.x < 0.0 { player_pos.x = b_max_x + player_radius; }
                                    velocity.x = 0.0;
                                }
                        }

                        player_pos.z += velocity.z;
                        for (b_min_x, b_max_x, b_min_y, b_max_y, b_min_z, b_max_z) in get_colliding_blocks(&chunks, player_pos, player_radius, player_height) {
                            if player_pos.x + player_radius > b_min_x && player_pos.x - player_radius < b_max_x &&
                                player_pos.y + player_height > b_min_y && player_pos.y < b_max_y &&
                                player_pos.z + player_radius > b_min_z && player_pos.z - player_radius < b_max_z {
                                    if velocity.z > 0.0 { player_pos.z = b_min_z - player_radius; }
                                    else if velocity.z < 0.0 { player_pos.z = b_max_z + player_radius; }
                                    velocity.z = 0.0;
                                }
                        }

                        let mut on_ground = false;
                        player_pos.y += velocity.y;
                        for (b_min_x, b_max_x, b_min_y, b_max_y, b_min_z, b_max_z) in get_colliding_blocks(&chunks, player_pos, player_radius, player_height) {
                            if player_pos.x + player_radius > b_min_x && player_pos.x - player_radius < b_max_x &&
                                player_pos.y + player_height > b_min_y && player_pos.y < b_max_y &&
                                player_pos.z + player_radius > b_min_z && player_pos.z - player_radius < b_max_z {
                                    if velocity.y < 0.0 { player_pos.y = b_max_y; on_ground = true; }
                                    else if velocity.y > 0.0 { player_pos.y = b_min_y - player_height; }
                                    velocity.y = 0.0;
                                }
                        }

                        if pressed_keys.contains(&KeyCode::Space) && on_ground && !in_water { velocity.y = 0.22; }

                        let current_cx = (player_pos.x / 16.0).floor() as i32;
                        let current_cz = (player_pos.z / 16.0).floor() as i32;

                        for cx in (current_cx - render_distance)..=(current_cx + render_distance) {
                            for cz in (current_cz - render_distance)..=(current_cz + render_distance) {
                                if !chunks.contains_key(&(cx, cz)) && !requested_chunks.contains(&(cx, cz)) {
                                    requested_chunks.insert((cx, cz));
                                    let world_dir_clone = world_dir.to_string();
                                    let tx = chunk_tx.clone();
                                    let chunks_snapshot = chunks.clone();
                                    let in_water_clone = in_water;
                                    thread::spawn(move || {
                                        let chunk = Chunk::load_mcr(&world_dir_clone, cx, cz).unwrap_or_else(|| {
                                            let new_chunk = Chunk::new_generated(cx, cz);
                                            new_chunk.save_mcr(&world_dir_clone);
                                            new_chunk
                                        });
                                        let mesh = chunk.generate_mesh(&chunks_snapshot, in_water_clone);
                                        let _ = tx.send((cx, cz, chunk, mesh));
                                    });
                                }
                            }
                        }

                        while let Ok((cx, cz, chunk, vertices)) = chunk_rx.try_recv() {
                            chunks.insert((cx, cz), chunk);

                            unsafe {
                                if let Some(gpu) = gpu_chunks.remove(&(cx, cz)) {
                                    gl.delete_vertex_array(gpu.vao);
                                    gl.delete_buffer(gpu.vbo);
                                }

                                let vao = gl.create_vertex_array().unwrap();
                                gl.bind_vertex_array(Some(vao));
                                let vbo = gl.create_buffer().unwrap();
                                gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
                                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&vertices), glow::STATIC_DRAW);

                                let stride = std::mem::size_of::<Vertex>() as i32;
                                gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
                                gl.enable_vertex_attrib_array(0);
                                gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 12);
                                gl.enable_vertex_attrib_array(1);
                                gl.bind_vertex_array(None);

                                gpu_chunks.insert((cx, cz), GpuChunk { vao, vbo, num_vertices: vertices.len() as i32 });
                            }
                        }

                        gpu_chunks.retain(|&(cx, cz), gpu| {
                            let keep = (cx - current_cx).abs() <= render_distance + 1 && (cz - current_cz).abs() <= render_distance + 1;
                            if !keep {
                                if let Some(chunk) = chunks.remove(&(cx, cz)) {
                                    chunk.save_mcr(world_dir);
                                }
                                requested_chunks.remove(&(cx, cz));
                                unsafe {
                                    gl.delete_vertex_array(gpu.vao);
                                    gl.delete_buffer(gpu.vbo);
                                }
                            }
                            keep
                        });

                        entities.retain_mut(|entity| {
                            let collected = entity.update(&chunks, player_pos);
                            !collected
                        });

                        // -----------------------------------------------------------------
                        // INTEGRACJA MOBÓW Z mobmodel.rs oraz mobs.rs
                        // -----------------------------------------------------------------
                        let mut events = Vec::new();
                        let ctx = Ctx {
                            chunks: &chunks,
                            player: player_pos,
                            player_alive: true,
                            day: true,
                        };

                        let mob_data_snapshot: Vec<(Vec3, MobType, bool)> = mobs.iter().map(|m| (m.pos, m.kind, m.dying)).collect();

                        for (i, mob) in mobs.iter_mut().enumerate() {
                            mob.update(&ctx, i, &mob_data_snapshot, &mut events);
                        }
                        mobs.retain(|m| !m.remove);

                        let eye_pos = player_pos + Vec3::new(0.0, eye_height, 0.0);
                        let aspect = window.inner_size().width as f32 / window.inner_size().height.max(1) as f32;
                        let projection = Mat4::perspective_rh(70.0_f32.to_radians(), aspect, 0.1, 100.0);
                        let view = Mat4::look_at_rh(eye_pos, eye_pos + camera_front, camera_up);
                        let view_proj = projection * view;
                        let targeted_block = raycast(&chunks, eye_pos, camera_front, 5.0);

                        let clear_color = if in_water { [0.08, 0.22, 0.45, 1.0] } else { [0.47, 0.65, 1.0, 1.0] };

                        unsafe {
                            gl.clear_color(clear_color[0], clear_color[1], clear_color[2], clear_color[3]);
                            gl.clear(glow::COLOR_BUFFER_BIT | glow::DEPTH_BUFFER_BIT);

                            gl.use_program(Some(shader_program));
                            let mvp_loc = gl.get_uniform_location(shader_program, "uMVP");
                            let tex_loc = gl.get_uniform_location(shader_program, "uTexture");
                            if let Some(l) = tex_loc { gl.uniform_1_i32(Some(&l), 0); }

                            gl.active_texture(glow::TEXTURE0);
                            gl.bind_texture(glow::TEXTURE_2D, Some(texture));

                            if let Some(l) = mvp_loc { gl.uniform_matrix_4_f32_slice(Some(&l), false, view_proj.as_ref()); }

                            for gpu in gpu_chunks.values() {
                                gl.bind_vertex_array(Some(gpu.vao));
                                gl.draw_arrays(glow::TRIANGLES, 0, gpu.num_vertices);
                            }

                            for entity in &entities {
                                let model_matrix = Mat4::from_translation(entity.pos) * Mat4::from_rotation_y(entity.rotation.to_radians());
                                let item_mvp = view_proj * model_matrix;
                                if let Some(l) = mvp_loc { gl.uniform_matrix_4_f32_slice(Some(&l), false, item_mvp.as_ref()); }

                                let item_verts = generate_item_mesh(entity.block_id);
                                gl.bind_buffer(glow::ARRAY_BUFFER, Some(item_vbo));
                                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&item_verts), glow::STATIC_DRAW);

                                gl.bind_vertex_array(Some(item_vao));
                                gl.draw_arrays(glow::TRIANGLES, 0, item_verts.len() as i32);
                            }

                            // Renderowanie mobów z użyciem modeli 3D z mobmodel.rs
                            for mob in &mobs {
                                let render_data = mob.render_data(0.5);
                                let defs = mobmodel::model(mob.kind);
                                let rots = mobmodel::pose(mob.kind, &defs, &render_data.pose);

                                for (def_idx, part_def) in defs.iter().enumerate() {
                                    let part_mat = render_data.root * mobmodel::part_matrix(part_def, rots[def_idx]);
                                    let mob_mvp = view_proj * part_mat;
                                    if let Some(l) = mvp_loc { gl.uniform_matrix_4_f32_slice(Some(&l), false, mob_mvp.as_ref()); }

                                    let part_verts = mobmodel::part_mesh(part_def);
                                    if !part_verts.is_empty() {
                                        gl.bind_buffer(glow::ARRAY_BUFFER, Some(item_vbo));
                                        gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&part_verts), glow::STATIC_DRAW);

                                        gl.bind_vertex_array(Some(item_vao));
                                        gl.draw_arrays(glow::TRIANGLES, 0, part_verts.len() as i32);
                                    }
                                }
                            }

                            if let Some((bx, by, bz, _, _, _)) = targeted_block {
                                if let Some(l) = mvp_loc { gl.uniform_matrix_4_f32_slice(Some(&l), false, view_proj.as_ref()); }
                                let outline_verts = get_wireframe_vertices(bx, by, bz);
                                gl.bind_buffer(glow::ARRAY_BUFFER, Some(outline_vbo));
                                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&outline_verts), glow::STATIC_DRAW);

                                gl.bind_vertex_array(Some(outline_vao));
                                gl.draw_arrays(glow::LINES, 0, 24);
                            }

                            let ortho = Mat4::orthographic_rh(-1.0, 1.0, -1.0, 1.0, -1.0, 1.0);
                            if let Some(l) = mvp_loc { gl.uniform_matrix_4_f32_slice(Some(&l), false, ortho.as_ref()); }

                            for (i, &b_id) in hotbar_blocks.iter().enumerate() {
                                let slot_x = -0.4 + (i as f32 * 0.1);
                                let slot_y = -0.85;

                                let is_selected = i == selected_slot;
                                let scale_factor = if is_selected { 1.2 } else { 1.0 };
                                let hud_matrix = Mat4::from_translation(Vec3::new(slot_x, slot_y, 0.0)) * Mat4::from_scale(Vec3::splat(scale_factor));
                                let hud_mvp = ortho * hud_matrix;
                                if let Some(l) = mvp_loc { gl.uniform_matrix_4_f32_slice(Some(&l), false, hud_mvp.as_ref()); }

                                let item_verts = generate_item_mesh(b_id);
                                gl.bind_buffer(glow::ARRAY_BUFFER, Some(item_vbo));
                                gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&item_verts), glow::STATIC_DRAW);

                                gl.bind_vertex_array(Some(item_vao));
                                gl.draw_arrays(glow::TRIANGLES, 0, item_verts.len() as i32);
                            }

                            gl.use_program(Some(crosshair_shader));
                            gl.bind_vertex_array(Some(crosshair_vao));
                            gl.draw_arrays(glow::LINES, 0, 4);

                            gl.bind_vertex_array(None);
                        }
                    }

                    gl_surface.swap_buffers(&gl_context).unwrap();
                    window.request_redraw();
                }
                _ => {}
            },
            Event::AboutToWait => { window.request_redraw(); }
            _ => {}
        }
    }).unwrap();
}

fn last_last_in_water_state(last: &mut bool, current: bool) -> bool {
    let prev = *last;
    *last = current;
    prev
}
