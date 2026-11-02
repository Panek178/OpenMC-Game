// Moby z Minecrafta Beta 1.7.3: Pig, Sheep, Cow, Chicken, Squid, Wolf, Creeper, Skeleton,
// Spider, Zombie, Slime, Ghast, Zombie Pigman, Giant. Logika działa w tickach 20 Hz.
use crate::mobmodel;
use crate::{Block, Chunk};
use glam::{Mat4, Vec3};
use std::collections::HashMap;
use std::sync::atomic::{AtomicU64, Ordering};

pub type Chunks = HashMap<(i32, i32), Chunk>;

// ---------- losowość ----------
static RNG: AtomicU64 = AtomicU64::new(0x1234_5678_9ABC_DEF1);
pub fn rnd() -> f32 {
    let mut z = RNG.fetch_add(0x9E37_79B9_7F4A_7C15, Ordering::Relaxed).wrapping_add(0x9E37_79B9_7F4A_7C15);
    z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
    z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
    z ^= z >> 31;
    ((z >> 40) as f32) / 16_777_216.0
}
pub fn rnd_i(n: i32) -> i32 { ((rnd() * n as f32) as i32).clamp(0, n.max(1) - 1) }

// ---------- przedmioty (pseudo-ID >= 200, rysowane jako kafelek wełny z tintem) ----------
pub const ITEM_PORK: Block = 200;
pub const ITEM_FEATHER: Block = 201;
pub const ITEM_LEATHER: Block = 202;
pub const ITEM_GUNPOWDER: Block = 203;
pub const ITEM_BONE: Block = 204;
pub const ITEM_ARROW: Block = 205;
pub const ITEM_STRING: Block = 206;
pub const ITEM_SLIME: Block = 207;
pub const ITEM_TEAR: Block = 208;
pub const ITEM_INK: Block = 209;
pub const ITEM_GOLD: Block = 210;
pub const ITEM_EGG: Block = 211;
pub const ITEM_COOKED: Block = 212;

pub fn item_tint(id: Block) -> [f32; 3] {
    match id {
        ITEM_PORK => [0.95, 0.6, 0.6],
        ITEM_FEATHER => [1.0, 1.0, 1.0],
        ITEM_LEATHER => [0.6, 0.38, 0.2],
        ITEM_GUNPOWDER => [0.35, 0.35, 0.35],
        ITEM_BONE => [0.92, 0.92, 0.85],
        ITEM_ARROW => [0.7, 0.55, 0.3],
        ITEM_STRING => [0.85, 0.85, 0.85],
        ITEM_SLIME => [0.4, 0.85, 0.4],
        ITEM_TEAR => [0.85, 0.9, 1.0],
        ITEM_INK => [0.08, 0.08, 0.12],
        ITEM_GOLD => [1.0, 0.85, 0.2],
        ITEM_EGG => [0.95, 0.9, 0.75],
        ITEM_COOKED => [0.7, 0.4, 0.25],
        _ => [1.0, 1.0, 1.0],
    }
}

// ---------- typy ----------
#[derive(Clone, Copy, PartialEq, Eq, Debug, Hash)]
pub enum MobType { Pig, Sheep, Cow, Chicken, Squid, Wolf, Creeper, Skeleton, Spider, Zombie, Slime, Ghast, PigZombie, Giant }

pub const ALL_MOBS: [MobType; 14] = [
    MobType::Pig, MobType::Sheep, MobType::Cow, MobType::Chicken, MobType::Squid, MobType::Wolf, MobType::Creeper,
MobType::Skeleton, MobType::Spider, MobType::Zombie, MobType::Slime, MobType::Ghast, MobType::PigZombie, MobType::Giant,
];

impl MobType {
    pub fn name(self) -> &'static str {
        match self {
            MobType::Pig => "Pig", MobType::Sheep => "Sheep", MobType::Cow => "Cow", MobType::Chicken => "Chicken",
            MobType::Squid => "Squid", MobType::Wolf => "Wolf", MobType::Creeper => "Creeper", MobType::Skeleton => "Skeleton",
            MobType::Spider => "Spider", MobType::Zombie => "Zombie", MobType::Slime => "Slime", MobType::Ghast => "Ghast",
            MobType::PigZombie => "Zombie Pigman", MobType::Giant => "Giant",
        }
    }
    pub fn is_passive(self) -> bool { matches!(self, MobType::Pig | MobType::Sheep | MobType::Cow | MobType::Chicken) }
    pub fn is_hostile(self) -> bool { matches!(self, MobType::Creeper | MobType::Skeleton | MobType::Spider | MobType::Zombie | MobType::Slime) }
}

/// (szerokość, wysokość) hitboxa
pub fn dims(kind: MobType, size: i32) -> (f32, f32) {
    match kind {
        MobType::Pig => (0.9, 0.9), MobType::Sheep => (0.9, 1.3), MobType::Cow => (0.9, 1.3), MobType::Chicken => (0.3, 0.4),
        MobType::Squid => (0.95, 0.95), MobType::Wolf => (0.8, 0.8), MobType::Creeper => (0.6, 1.8), MobType::Skeleton => (0.6, 1.8),
        MobType::Spider => (1.4, 0.9), MobType::Zombie => (0.6, 1.8), MobType::PigZombie => (0.6, 1.8),
        MobType::Slime => (0.6 * size as f32, 0.6 * size as f32), MobType::Ghast => (4.0, 4.0), MobType::Giant => (3.6, 10.8),
    }
}

fn max_health(kind: MobType, size: i32) -> f32 {
    match kind {
        MobType::Pig | MobType::Cow | MobType::Squid | MobType::Ghast => 10.0,
        MobType::Sheep | MobType::Wolf => 8.0,
        MobType::Chicken => 4.0,
        MobType::Spider => 16.0,
        MobType::Slime => (size * size) as f32,
        MobType::Giant => 100.0,
        _ => 20.0,
    }
}

// ---------- świat ----------
pub fn blk(chunks: &Chunks, x: i32, y: i32, z: i32) -> Block {
    if y < 0 { return 1; }
    if y >= 128 { return 0; }
    if !chunks.contains_key(&(x.div_euclid(16), z.div_euclid(16))) { return 1; } // niezaładowane = lita skała
    Chunk::get_block_global(chunks, x, y, z)
}
pub fn solid(b: Block) -> bool { b != 0 && b != 8 && b != 9 }
pub fn liquid(b: Block) -> bool { b == 8 || b == 9 }
fn bi(c: f32) -> i32 { (c + 0.5).floor() as i32 }

pub fn sky_visible(chunks: &Chunks, x: i32, y: i32, z: i32) -> bool {
    for yy in (y + 1)..128 { if blk(chunks, x, yy, z) != 0 { return false; } }
    true
}

pub fn line_of_sight(chunks: &Chunks, a: Vec3, b: Vec3) -> bool {
    let d = b - a;
    let len = d.length();
    if len < 0.01 { return true; }
    let dir = d / len;
    let mut t = 0.4;
    while t < len {
        let p = a + dir * t;
        if solid(blk(chunks, bi(p.x), bi(p.y), bi(p.z))) { return false; }
        t += 0.4;
    }
    true
}

fn collides(chunks: &Chunks, p: Vec3, hw: f32, h: f32) -> bool {
    let e = 0.001;
    for x in bi(p.x - hw + e)..=bi(p.x + hw - e) {
        for y in bi(p.y + e)..=bi(p.y + h - e) {
            for z in bi(p.z - hw + e)..=bi(p.z + hw - e) {
                if solid(blk(chunks, x, y, z)) { return true; }
            }
        }
    }
    false
}

fn move_axis(chunks: &Chunks, pos: &mut Vec3, d: f32, axis: usize, hw: f32, h: f32) -> bool {
    if d == 0.0 { return false; }
    let mut np = *pos;
    np[axis] += d;
    if !collides(chunks, np, hw, h) { *pos = np; return false; }
    let (mut lo, mut hi) = (0.0f32, 1.0f32);
    for _ in 0..8 {
        let mid = (lo + hi) * 0.5;
        let mut t = *pos;
        t[axis] += d * mid;
        if collides(chunks, t, hw, h) { hi = mid; } else { lo = mid; }
    }
    pos[axis] += d * lo;
    true
}

pub fn heading(yaw_deg: f32) -> Vec3 { let r = yaw_deg.to_radians(); Vec3::new(-r.sin(), 0.0, r.cos()) }
fn approach_angle(cur: f32, target: f32, step: f32) -> f32 {
    let mut d = (target - cur) % 360.0;
    if d > 180.0 { d -= 360.0; }
    if d < -180.0 { d += 360.0; }
    cur + d.clamp(-step, step)
}

// ---------- zdarzenia ----------
pub enum Ev {
    HurtPlayer { amount: f32, from: Vec3 },
    Explode { pos: Vec3, power: f32 },
    Arrow { pos: Vec3, vel: Vec3 },
    Fireball { pos: Vec3, vel: Vec3 },
    SpawnSlime { pos: Vec3, size: i32 },
    HurtMob { idx: usize, amount: f32, from: Vec3 },
    Drop { pos: Vec3, item: Block },
    Sound(&'static str),
}

pub struct Ctx<'a> {
    pub chunks: &'a Chunks,
    pub player: Vec3,
    pub player_alive: bool,
    pub day: bool,
}

pub fn sound_say(kind: MobType) -> Option<&'static str> {
    Some(match kind {
        MobType::Pig => "assets/sound/mob/pig1.ogg", MobType::Cow => "assets/sound/mob/cow1.ogg",
         MobType::Sheep => "assets/sound/mob/sheep1.ogg", MobType::Chicken => "assets/sound/mob/chicken1.ogg",
         MobType::Zombie | MobType::Giant => "assets/sound/mob/zombie1.ogg", MobType::PigZombie => "assets/sound/mob/zombiepig/zpig1.ogg",
         MobType::Skeleton => "assets/sound/mob/skeleton1.ogg", MobType::Spider => "assets/sound/mob/spider1.ogg",
         MobType::Ghast => "assets/sound/mob/ghast/moan1.ogg", MobType::Wolf => "assets/sound/mob/wolf/growl1.ogg",
         _ => return None,
    })
}
pub fn sound_hurt(kind: MobType) -> Option<&'static str> {
    Some(match kind {
        MobType::Pig => "assets/sound/mob/pig1.ogg", MobType::Cow => "assets/sound/mob/cowhurt1.ogg",
         MobType::Sheep => "assets/sound/mob/sheep1.ogg", MobType::Chicken => "assets/sound/mob/chickenhurt1.ogg",
         MobType::Zombie | MobType::Giant => "assets/sound/mob/zombiehurt1.ogg", MobType::PigZombie => "assets/sound/mob/zombiepig/zpighurt1.ogg",
         MobType::Skeleton => "assets/sound/mob/skeletonhurt1.ogg", MobType::Spider => "assets/sound/mob/spider1.ogg",
         MobType::Ghast => "assets/sound/mob/ghast/scream1.ogg", MobType::Wolf => "assets/sound/mob/wolf/hurt1.ogg",
         MobType::Creeper => "assets/sound/mob/creeper1.ogg", MobType::Slime => "assets/sound/mob/slime1.ogg",
         _ => return None,
    })
}

// ---------- mob ----------
pub struct Mob {
    pub kind: MobType,
    pub size: i32,
    pub pos: Vec3,
    pub prev_pos: Vec3,
    pub vel: Vec3,
    pub yaw: f32,
    pub prev_yaw: f32,
    pub health: f32,
    pub on_ground: bool,
    pub in_water: bool,
    pub hit_wall: bool,
    pub fall: f32,
    pub age: u32,
    pub invuln: i32,
    pub hurt_timer: i32,
    pub knock: i32,
    pub attack_cd: i32,
    pub swell: i32,
    pub prev_swell: i32,
    pub angry: i32,
    pub panic: i32,
    pub panic_from: Vec3,
    pub burn: i32,
    pub dying: bool,
    pub death_timer: i32,
    pub remove: bool,
    pub wander_timer: i32,
    pub wander_yaw: f32,
    pub wander_move: bool,
    pub jump_timer: i32,
    pub egg_timer: i32,
    pub swim_dir: Vec3,
    pub charging: bool,
    pub has_target: bool,
    pub alert: bool,
    pub move_dir: Vec3,
    pub move_speed: f32,
    pub head_pitch: f32,
    pub amt: f32,
    pub prev_amt: f32,
    pub swing: f32,
    pub prev_swing: f32,
    pub fleece: [f32; 3],
}

const FLEECE: [[f32; 3]; 16] = [
    [1.0, 1.0, 1.0], [0.85, 0.5, 0.2], [0.7, 0.3, 0.85], [0.4, 0.6, 0.85], [0.9, 0.9, 0.2], [0.5, 0.8, 0.1], [0.95, 0.5, 0.65],
[0.3, 0.3, 0.3], [0.6, 0.6, 0.6], [0.3, 0.5, 0.6], [0.5, 0.25, 0.7], [0.2, 0.3, 0.7], [0.4, 0.3, 0.2], [0.4, 0.5, 0.2],
[0.6, 0.2, 0.2], [0.1, 0.1, 0.1],
];

impl Mob {
    pub fn new(pos: Vec3, kind: MobType) -> Self { Self::new_sized(pos, kind, 1) }

    pub fn new_sized(pos: Vec3, kind: MobType, size: i32) -> Self {
        let r = rnd_i(100);
        let fleece = if r < 5 { FLEECE[15] } else if r < 10 { FLEECE[7] } else if r < 15 { FLEECE[8] } else if r < 18 { FLEECE[12] }
        else if rnd_i(500) == 0 { FLEECE[6] } else { FLEECE[0] };
        let yaw = rnd() * 360.0;
        Self {
            kind, size, pos, prev_pos: pos, vel: Vec3::ZERO, yaw, prev_yaw: yaw, health: max_health(kind, size),
            on_ground: false, in_water: false, hit_wall: false, fall: 0.0, age: 0, invuln: 0, hurt_timer: 0, knock: 0,
            attack_cd: 0, swell: 0, prev_swell: 0, angry: 0, panic: 0, panic_from: pos, burn: 0, dying: false, death_timer: 0,
            remove: false, wander_timer: rnd_i(60), wander_yaw: yaw, wander_move: false, jump_timer: 20 + rnd_i(20),
            egg_timer: 6000 + rnd_i(6000), swim_dir: Vec3::ZERO, charging: false, has_target: false, alert: false,
            move_dir: Vec3::ZERO, move_speed: 0.0, head_pitch: 0.0, amt: 0.0, prev_amt: 0.0, swing: 0.0, prev_swing: 0.0, fleece,
        }
    }

    pub fn bbox(&self) -> (Vec3, Vec3) {
        let (w, h) = dims(self.kind, self.size);
        let hw = w / 2.0 + 0.05;
        (Vec3::new(self.pos.x - hw, self.pos.y - 0.05, self.pos.z - hw), Vec3::new(self.pos.x + hw, self.pos.y + h + 0.05, self.pos.z + hw))
    }
    pub fn center(&self) -> Vec3 { let (_, h) = dims(self.kind, self.size); self.pos + Vec3::new(0.0, h * 0.5, 0.0) }

    fn set_move(&mut self, dir: Vec3, speed: f32) { self.move_dir = dir; self.move_speed = speed; }
    fn face_dir(&mut self, d: Vec3) {
        if d.x.abs() + d.z.abs() > 1e-4 { self.yaw = approach_angle(self.yaw, (-d.x).atan2(d.z).to_degrees(), 30.0); }
    }
    fn chase(&mut self, target: Vec3, speed: f32) {
        let d = Vec3::new(target.x - self.pos.x, 0.0, target.z - self.pos.z);
        self.set_move(d.normalize_or_zero(), speed);
        self.face_dir(d);
    }
    fn wander(&mut self, speed: f32) {
        self.wander_timer -= 1;
        if self.wander_timer <= 0 {
            self.wander_timer = 40 + rnd_i(100);
            if rnd() < 0.4 { self.wander_move = false; } else { self.wander_move = true; self.wander_yaw = rnd() * 360.0; }
        }
        if self.wander_move {
            self.yaw = approach_angle(self.yaw, self.wander_yaw, 15.0);
            self.set_move(heading(self.wander_yaw), speed);
        } else {
            self.set_move(Vec3::ZERO, 0.0);
        }
    }
    fn passive_ai(&mut self, speed: f32) {
        if self.panic > 0 {
            self.panic -= 1;
            let d = Vec3::new(self.pos.x - self.panic_from.x, 0.0, self.pos.z - self.panic_from.z).normalize_or_zero();
            self.set_move(d, speed * 2.0);
            self.face_dir(d);
        } else {
            self.wander(speed);
        }
    }
    fn melee(&mut self, ctx: &Ctx, dmg: f32, evs: &mut Vec<Ev>) {
        let (w, h) = dims(self.kind, self.size);
        let dx = ctx.player.x - self.pos.x;
        let dz = ctx.player.z - self.pos.z;
        let hd = (dx * dx + dz * dz).sqrt();
        if self.attack_cd <= 0 && hd < w / 2.0 + 0.9 && ctx.player.y + 1.8 > self.pos.y && ctx.player.y < self.pos.y + h {
            evs.push(Ev::HurtPlayer { amount: dmg, from: self.pos });
            self.attack_cd = 20;
        }
    }

    pub fn hurt(&mut self, amount: f32, from: Vec3, by_player: bool, evs: &mut Vec<Ev>) -> bool {
        if self.dying || self.invuln > 0 { return false; }
        self.invuln = 10;
        self.knock = 8;
        let d = Vec3::new(self.pos.x - from.x, 0.0, self.pos.z - from.z).normalize_or_zero();
        let k = if self.kind == MobType::Ghast { 0.0 } else { 0.4 };
        self.vel.x = d.x * k;
        self.vel.z = d.z * k;
        if self.kind != MobType::Ghast { self.vel.y = 0.32; }
        if self.kind.is_passive() { self.panic = 60; self.panic_from = from; }
        if by_player {
            match self.kind {
                MobType::Wolf => { self.angry = 600; self.alert = true; }
                MobType::PigZombie => { self.angry = 400 + rnd_i(400); self.alert = true; }
                MobType::Spider => { self.angry = 300; }
                _ => {}
            }
        }
        if let Some(s) = sound_hurt(self.kind) { evs.push(Ev::Sound(s)); }
        self.take(amount, evs);
        true
    }

    fn take(&mut self, amount: f32, evs: &mut Vec<Ev>) {
        self.health -= amount;
        self.hurt_timer = 10;
        if self.health <= 0.0 && !self.dying {
            self.dying = true;
            self.death_timer = 0;
            self.drop_items(evs);
            if self.kind == MobType::Slime && self.size > 1 {
                for _ in 0..(2 + rnd_i(3)) {
                    let off = Vec3::new((rnd() - 0.5) * self.size as f32 * 0.5, 0.0, (rnd() - 0.5) * self.size as f32 * 0.5);
                    evs.push(Ev::SpawnSlime { pos: self.pos + off, size: self.size / 2 });
                }
            }
        }
    }

    fn drop_items(&self, evs: &mut Vec<Ev>) {
        let c = self.pos + Vec3::new(0.0, 0.5, 0.0);
        let mut add = |id: Block, n: i32| { for _ in 0..n { evs.push(Ev::Drop { pos: c, item: id }); } };
        match self.kind {
            MobType::Pig => add(ITEM_PORK, rnd_i(3)),
            MobType::Cow => add(ITEM_LEATHER, rnd_i(3)),
            MobType::Chicken => add(ITEM_FEATHER, rnd_i(3)),
            MobType::Sheep => add(35, 1),
            MobType::Squid => add(ITEM_INK, 1 + rnd_i(3)),
            MobType::Creeper => add(ITEM_GUNPOWDER, rnd_i(3)),
            MobType::Skeleton => { add(ITEM_ARROW, rnd_i(3)); add(ITEM_BONE, rnd_i(3)); }
            MobType::Spider => add(ITEM_STRING, rnd_i(3)),
            MobType::Zombie => add(ITEM_FEATHER, rnd_i(3)),
            MobType::Slime => { if self.size == 1 { add(ITEM_SLIME, rnd_i(3)); } }
            MobType::Ghast => { add(ITEM_TEAR, rnd_i(2)); add(ITEM_GUNPOWDER, rnd_i(3)); }
            MobType::PigZombie => { add(ITEM_COOKED, rnd_i(2)); add(ITEM_GOLD, rnd_i(2)); }
            MobType::Wolf | MobType::Giant => {}
        }
    }

    pub fn update(&mut self, ctx: &Ctx, idx: usize, others: &[(Vec3, MobType, bool)], evs: &mut Vec<Ev>) {
        self.prev_pos = self.pos;
        self.prev_yaw = self.yaw;
        self.prev_swell = self.swell;
        self.age += 1;
        if self.invuln > 0 { self.invuln -= 1; }
        if self.hurt_timer > 0 { self.hurt_timer -= 1; }
        if self.attack_cd > 0 { self.attack_cd -= 1; }
        if self.angry > 0 { self.angry -= 1; }

        if self.dying {
            self.death_timer += 1;
            self.set_move(Vec3::ZERO, 0.0);
            self.physics(ctx.chunks, evs);
            if self.death_timer >= 20 { self.remove = true; }
            return;
        }

        let (_, h) = dims(self.kind, self.size);
        self.in_water = liquid(blk(ctx.chunks, bi(self.pos.x), bi(self.pos.y + h * 0.4), bi(self.pos.z)));

        if matches!(self.kind, MobType::Zombie | MobType::Skeleton) && ctx.day && !self.in_water
            && sky_visible(ctx.chunks, bi(self.pos.x), bi(self.pos.y + h), bi(self.pos.z)) {
                self.burn = 160;
            }
            if self.burn > 0 {
                self.burn -= 1;
                if self.in_water { self.burn = 0; } else if self.burn % 20 == 0 { self.take(1.0, evs); }
                if self.dying { return; }
            }

            self.ai(ctx, idx, others, evs);
        self.physics(ctx.chunks, evs);

        let dx = self.pos.x - self.prev_pos.x;
        let dz = self.pos.z - self.prev_pos.z;
        let f = ((dx * dx + dz * dz).sqrt() * 4.0).min(1.0);
        self.prev_amt = self.amt;
        self.prev_swing = self.swing;
        self.amt += (f - self.amt) * 0.4;
        self.swing += self.amt;
    }

    fn ai(&mut self, ctx: &Ctx, _idx: usize, others: &[(Vec3, MobType, bool)], evs: &mut Vec<Ev>) {
        let center = self.center();
        let to_p = ctx.player + Vec3::new(0.0, 1.0, 0.0) - center;
        let dist = to_p.length();
        let hdist = (to_p.x * to_p.x + to_p.z * to_p.z).sqrt();
        let see = ctx.player_alive && dist < 16.0;
        self.has_target = false;
        if rnd() < 0.004 { if let Some(s) = sound_say(self.kind) { if dist < 16.0 { evs.push(Ev::Sound(s)); } } }
        let pitch_to = |tp: Vec3| (-tp.y).atan2((tp.x * tp.x + tp.z * tp.z).sqrt());

        match self.kind {
            MobType::Pig => self.passive_ai(0.06),
            MobType::Cow => self.passive_ai(0.05),
            MobType::Sheep => self.passive_ai(0.055),
            MobType::Chicken => {
                self.passive_ai(0.06);
                self.egg_timer -= 1;
                if self.egg_timer <= 0 {
                    self.egg_timer = 6000 + rnd_i(6000);
                    evs.push(Ev::Drop { pos: self.pos + Vec3::new(0.0, 0.3, 0.0), item: ITEM_EGG });
                }
            }
            MobType::Zombie | MobType::Giant => {
                if see {
                    self.has_target = true;
                    self.chase(ctx.player, if self.kind == MobType::Giant { 0.1 } else { 0.14 });
                    self.head_pitch = pitch_to(to_p);
                    let dmg = if self.kind == MobType::Giant { 50.0 } else { 5.0 };
                    self.melee(ctx, dmg, evs);
                } else { self.head_pitch *= 0.9; self.wander(0.05); }
            }
            MobType::PigZombie => {
                if self.angry > 0 && see {
                    self.has_target = true;
                    self.chase(ctx.player, 0.18);
                    self.head_pitch = pitch_to(to_p);
                    self.melee(ctx, 5.0, evs);
                } else { self.head_pitch *= 0.9; self.wander(0.05); }
            }
            MobType::Skeleton => {
                if see {
                    self.has_target = true;
                    let d = Vec3::new(to_p.x, 0.0, to_p.z);
                    self.face_dir(d);
                    self.head_pitch = pitch_to(to_p);
                    let los = line_of_sight(ctx.chunks, center + Vec3::new(0.0, 0.6, 0.0), ctx.player + Vec3::new(0.0, 1.5, 0.0));
                    if hdist > 9.0 || !los { self.set_move(d.normalize_or_zero(), 0.11); } else { self.set_move(Vec3::ZERO, 0.0); }
                    if los && hdist < 12.0 && self.attack_cd <= 0 {
                        self.attack_cd = 40 + rnd_i(20);
                        let eye = self.pos + Vec3::new(0.0, 1.5, 0.0);
                        let target = ctx.player + Vec3::new(0.0, 1.3, 0.0);
                        let mut dir = target - eye;
                        dir.y += hdist * 0.12;
                        let dir = (dir.normalize_or_zero() + Vec3::new(rnd() - 0.5, rnd() - 0.5, rnd() - 0.5) * 0.08).normalize_or_zero();
                        evs.push(Ev::Arrow { pos: eye, vel: dir * 1.6 });
                        evs.push(Ev::Sound("assets/sound/random/bow.ogg"));
                    }
                } else { self.head_pitch *= 0.9; self.wander(0.05); }
            }
            MobType::Creeper => {
                let near = see && dist < 3.0 && line_of_sight(ctx.chunks, center, ctx.player + Vec3::new(0.0, 1.0, 0.0));
                if see {
                    self.has_target = true;
                    let d = Vec3::new(to_p.x, 0.0, to_p.z);
                    self.face_dir(d);
                    self.head_pitch = pitch_to(to_p);
                    if near || (self.swell > 0 && dist < 7.0) {
                        self.set_move(Vec3::ZERO, 0.0);
                    } else {
                        self.set_move(d.normalize_or_zero(), 0.14);
                    }
                } else { self.set_move(Vec3::ZERO, 0.0); self.wander(0.05); }
                if near || (self.swell > 0 && dist < 7.0 && see) {
                    if self.swell == 0 { evs.push(Ev::Sound("assets/sound/random/fuse.ogg")); }
                    self.swell += 1;
                    if self.swell >= 30 {
                        evs.push(Ev::Explode { pos: center, power: 3.0 });
                        self.remove = true;
                    }
                } else if self.swell > 0 { self.swell -= 1; }
            }
            MobType::Spider => {
                let hostile = see && (!ctx.day || self.angry > 0);
                if hostile {
                    self.has_target = true;
                    let d = Vec3::new(to_p.x, 0.0, to_p.z);
                    self.face_dir(d);
                    self.head_pitch = pitch_to(to_p);
                    if self.on_ground && dist > 2.0 && dist < 6.0 && rnd() < 0.04 {
                        let dir = d.normalize_or_zero();
                        self.vel = Vec3::new(dir.x * 0.45, 0.4, dir.z * 0.45);
                        self.set_move(Vec3::ZERO, 0.0);
                        self.knock = 6;
                    } else {
                        self.set_move(d.normalize_or_zero(), 0.2);
                    }
                    self.melee(ctx, 2.0, evs);
                } else { self.wander(0.06); }
            }
            MobType::Slime => {
                if see { self.has_target = true; }
                if self.on_ground {
                    self.set_move(Vec3::ZERO, 0.0);
                    if self.jump_timer > 0 { self.jump_timer -= 1; } else {
                        self.jump_timer = if see { 10 + rnd_i(10) } else { 20 + rnd_i(40) };
                        self.vel.y = 0.42;
                        let d = if see { Vec3::new(to_p.x, 0.0, to_p.z).normalize_or_zero() } else { heading(rnd() * 360.0) };
                        self.face_dir(d);
                        self.set_move(d, 0.1);
                        self.vel.x = d.x * 0.1;
                        self.vel.z = d.z * 0.1;
                        self.knock = 4;
                        evs.push(Ev::Sound("assets/sound/mob/slime1.ogg"));
                    }
                }
                if self.size > 1 && see { self.melee(ctx, self.size as f32, evs); }
            }
            MobType::Wolf => {
                if self.angry > 0 && see {
                    self.has_target = true;
                    self.chase(ctx.player, 0.18);
                    self.head_pitch = pitch_to(to_p);
                    self.melee(ctx, 2.0, evs);
                } else {
                    let mut best: Option<(usize, f32)> = None;
                    for (i, (p, k, dead)) in others.iter().enumerate() {
                        if *dead || *k != MobType::Sheep { continue; }
                        let d = (*p - self.pos).length();
                        if d < 16.0 && best.map_or(true, |(_, bd)| d < bd) { best = Some((i, d)); }
                    }
                    if let Some((i, d)) = best {
                        let sp = others[i].0;
                        self.chase(sp, 0.17);
                        if d < 1.4 && self.attack_cd <= 0 {
                            self.attack_cd = 20;
                            evs.push(Ev::HurtMob { idx: i, amount: 2.0, from: self.pos });
                        }
                    } else { self.passive_ai(0.1); }
                }
            }
            MobType::Squid => {
                if self.in_water {
                    if self.swim_dir == Vec3::ZERO || rnd() < 0.01 || self.age % 100 == 0 {
                        self.swim_dir = Vec3::new(rnd() - 0.5, (rnd() - 0.5) * 0.4, rnd() - 0.5).normalize_or_zero();
                    }
                    let probe = center + self.swim_dir * 1.2;
                    if !liquid(blk(ctx.chunks, bi(probe.x), bi(probe.y), bi(probe.z))) {
                        self.swim_dir = Vec3::new(rnd() - 0.5, -0.2, rnd() - 0.5).normalize_or_zero();
                    }
                    let sp = 0.06 + 0.03 * ((self.age as f32 * 0.2).sin() + 1.0);
                    self.vel = self.vel * 0.8 + self.swim_dir * sp * 0.2;
                    self.face_dir(Vec3::new(self.swim_dir.x, 0.0, self.swim_dir.z));
                }
            }
            MobType::Ghast => {
                self.has_target = false;
                if self.wander_timer <= 0 || self.age % 120 == 0 {
                    self.wander_timer = 100;
                    self.swim_dir = Vec3::new(rnd() - 0.5, (rnd() - 0.5) * 0.6, rnd() - 0.5).normalize_or_zero();
                }
                self.wander_timer -= 1;
                self.vel += self.swim_dir * 0.01;
                if self.hit_wall { self.swim_dir = -self.swim_dir; }
                let los = ctx.player_alive && dist < 64.0 && line_of_sight(ctx.chunks, center, ctx.player + Vec3::new(0.0, 1.0, 0.0));
                if los {
                    self.face_dir(Vec3::new(to_p.x, 0.0, to_p.z));
                    if self.attack_cd <= 0 { self.attack_cd = 60; }
                    self.charging = self.attack_cd <= 20;
                    if self.attack_cd == 1 {
                        evs.push(Ev::Fireball { pos: center + heading(self.yaw) * 2.2, vel: to_p.normalize_or_zero() * 0.9 });
                        evs.push(Ev::Sound("assets/sound/mob/ghast/fireball4.ogg"));
                        self.charging = false;
                    }
                } else { self.charging = false; if self.attack_cd > 20 { self.attack_cd = 20; } }
                if self.age % 100 == 0 && los { evs.push(Ev::Sound("assets/sound/mob/ghast/moan1.ogg")); }
            }
        }
    }

    fn physics(&mut self, chunks: &Chunks, evs: &mut Vec<Ev>) {
        let (w, h) = dims(self.kind, self.size);
        let hw = w / 2.0;
        let flying = self.kind == MobType::Ghast || (self.kind == MobType::Squid && self.in_water);

        if self.kind == MobType::Ghast {
            self.vel *= 0.95;
        } else if flying {
            // zwisanie/pływanie obsługiwane w AI
        } else {
            if self.kind == MobType::Spider && self.hit_wall && self.has_target { self.vel.y = 0.2; self.fall = 0.0; }
            else if self.in_water { self.vel.y = self.vel.y * 0.8 + 0.012; }
            else { self.vel.y = (self.vel.y - 0.08) * 0.98; }
            if self.in_water && self.hit_wall && self.move_speed > 0.0 { self.vel.y = 0.3; }
            if self.kind == MobType::Chicken && self.vel.y < -0.1 { self.vel.y = -0.1; self.fall = 0.0; }
            if self.vel.y < -3.0 { self.vel.y = -3.0; }

            let tgt = self.move_dir * self.move_speed * if self.in_water { 0.6 } else { 1.0 };
            if self.knock > 0 {
                self.knock -= 1;
                self.vel.x *= 0.9;
                self.vel.z *= 0.9;
            } else if self.on_ground || self.in_water {
                self.vel.x = tgt.x;
                self.vel.z = tgt.z;
            } else {
                self.vel.x = self.vel.x * 0.91 + tgt.x * 0.09;
                self.vel.z = self.vel.z * 0.91 + tgt.z * 0.09;
            }
        }

        let v = self.vel;
        let old_y = self.pos.y;
        let mut p = self.pos;
        let hy = move_axis(chunks, &mut p, v.y, 1, hw, h);
        let hx = move_axis(chunks, &mut p, v.x, 0, hw, h);
        let hz = move_axis(chunks, &mut p, v.z, 2, hw, h);
        self.pos = p;
        self.hit_wall = hx || hz;
        if hx { self.vel.x = 0.0; }
        if hz { self.vel.z = 0.0; }
        self.on_ground = hy && v.y < 0.0;
        if hy { self.vel.y = 0.0; }

        if self.pos.y < old_y && !self.on_ground { self.fall += old_y - self.pos.y; }
        if self.in_water || flying { self.fall = 0.0; }
        if self.on_ground {
            if self.fall > 3.0 && !matches!(self.kind, MobType::Chicken | MobType::Ghast | MobType::Squid) && !self.dying {
                self.take((self.fall - 3.0).ceil(), evs);
            }
            self.fall = 0.0;
            if self.hit_wall && self.move_speed > 0.0 && !self.in_water && self.kind != MobType::Slime && !self.dying {
                self.vel.y = 0.42;
            }
        }
        if self.pos.y < -20.0 { self.remove = true; }
    }

    pub fn render_data(&self, alpha: f32) -> MobRender {
        let pos = self.prev_pos.lerp(self.pos, alpha);
        let yaw = approach_angle(self.prev_yaw, self.yaw, 360.0);
        let mut root = Mat4::from_translation(pos) * Mat4::from_rotation_y(-yaw.to_radians());
        if self.dying {
            let t = (self.death_timer as f32 + alpha) / 20.0 * 1.6;
            root = root * Mat4::from_rotation_z(t.sqrt().min(1.0) * std::f32::consts::FRAC_PI_2);
        }
        let mut add = [0.0f32; 3];
        let mut tint = [1.0f32; 3];
        match self.kind {
            MobType::Slime => root = root * Mat4::from_scale(Vec3::splat(self.size as f32)),
            MobType::Ghast => root = root * Mat4::from_translation(Vec3::new(0.0, -0.25, 0.0)) * Mat4::from_scale(Vec3::splat(4.5)),
            MobType::Giant => root = root * Mat4::from_scale(Vec3::splat(6.0)),
            MobType::Creeper => {
                let sw = ((self.prev_swell as f32) + (self.swell - self.prev_swell) as f32 * alpha) / 28.0;
                let sw = sw.clamp(0.0, 1.0);
                let wob = 1.0 + (sw * 100.0).sin() * sw * 0.01;
                let s2 = sw * sw * sw * sw;
                root = root * Mat4::from_scale(Vec3::new((1.0 + s2 * 0.4) * wob, (1.0 + s2 * 0.1) / wob, (1.0 + s2 * 0.4) * wob));
                if self.swell > 0 && ((sw * 10.0) as i32) % 2 == 0 { add = [0.5, 0.5, 0.5]; }
            }
            MobType::Squid => {
                root = Mat4::from_translation(pos + Vec3::new(0.0, 0.475, 0.0)) * Mat4::from_rotation_y(-yaw.to_radians())
                * Mat4::from_translation(Vec3::new(0.0, -1.0, 0.0));
            }
            _ => {}
        }
        if self.hurt_timer > 0 || self.dying { tint = [1.0, 0.4, 0.4]; }
        if self.burn > 0 { add = [0.35, 0.18, 0.0]; }
        let a = self.prev_amt + (self.amt - self.prev_amt) * alpha;
        let sw = self.prev_swing + (self.swing - self.prev_swing) * alpha;
        let flap = match self.kind {
            MobType::Chicken => if self.on_ground { 0.0 } else { ((self.age as f32 + alpha) * 1.0).sin().abs() * 1.0 + 0.3 },
            MobType::Squid => ((self.age as f32 + alpha) * 0.25).sin() * 0.4 + 0.3,
            _ => 0.0,
        };
        let pose = mobmodel::Pose {
            swing: sw, amt: a, head_yaw: 0.0, head_pitch: self.head_pitch, age: self.age as f32 + alpha,
            arms_up: matches!(self.kind, MobType::Zombie | MobType::PigZombie | MobType::Giant) && self.has_target || self.kind == MobType::Skeleton && self.has_target,
            flap,
        };
        let angry = self.kind == MobType::Wolf && self.angry > 0;
        MobRender {
            root, pose, skins: mobmodel::skins(self.kind, angry, self.charging), tint, add,
            fur_tint: if self.hurt_timer > 0 || self.dying { [self.fleece[0], self.fleece[1] * 0.4, self.fleece[2] * 0.4] } else { self.fleece },
        }
    }
}

pub struct MobRender {
    pub root: Mat4,
    pub pose: mobmodel::Pose,
    pub skins: [&'static str; 2],
    pub tint: [f32; 3],
    pub fur_tint: [f32; 3],
    pub add: [f32; 3],
}
