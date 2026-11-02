use crate::mobs::MobType;
use crate::Vertex;
use glam::{Mat4, Vec3};

pub struct BoxDef {
    pub tex: (f32, f32),
    pub from: [f32; 3],
    pub size: [f32; 3],
    pub inflate: f32,
    pub mirror: bool,
}

pub struct PartDef {
    pub point: [f32; 3],
    pub rot: [f32; 3],
    pub boxes: Vec<BoxDef>,
    pub skin: u8,
}

fn bx(tu: f32, tv: f32, x: f32, y: f32, z: f32, w: f32, h: f32, d: f32) -> BoxDef {
    BoxDef { tex: (tu, tv), from: [x, y, z], size: [w, h, d], inflate: 0.0, mirror: false }
}
fn bxi(mut b: BoxDef, inflate: f32) -> BoxDef { b.inflate = inflate; b }
fn mir(mut b: BoxDef) -> BoxDef { b.mirror = true; b }
fn part(point: [f32; 3], boxes: Vec<BoxDef>) -> PartDef { PartDef { point, rot: [0.0; 3], boxes, skin: 0 } }
fn part_rx(point: [f32; 3], rx: f32, boxes: Vec<BoxDef>) -> PartDef { PartDef { point, rot: [rx, 0.0, 0.0], boxes, skin: 0 } }
fn overlay(mut p: PartDef) -> PartDef { p.skin = 1; p }

const HALF_PI: f32 = std::f32::consts::FRAC_PI_2;
const PI: f32 = std::f32::consts::PI;

pub fn model(kind: MobType) -> Vec<PartDef> {
    use MobType::*;
    match kind {
        Zombie | Skeleton | PigZombie | Giant => vec![
            part([0.0, 0.0, 0.0], vec![bx(0., 0., -4., -8., -4., 8., 8., 8.)]),
            part([0.0, 0.0, 0.0], vec![bx(16., 16., -4., 0., -2., 8., 12., 4.)]),
            part([-5.0, 2.0, 0.0], vec![bx(40., 16., -3., -2., -2., 4., 12., 4.)]),
            part([5.0, 2.0, 0.0], vec![mir(bx(40., 16., -1., -2., -2., 4., 12., 4.))]),
            part([-2.0, 12.0, 0.0], vec![bx(0., 16., -2., 0., -2., 4., 12., 4.)]),
            part([2.0, 12.0, 0.0], vec![mir(bx(0., 16., -2., 0., -2., 4., 12., 4.))]),
        ],
        Creeper => vec![
            part([0.0, 6.0, 0.0], vec![bx(0., 0., -4., -8., -4., 8., 8., 8.)]),
            part([0.0, 6.0, 0.0], vec![bx(16., 16., -4., 0., -2., 8., 12., 4.)]),
            part([-2.0, 18.0, 4.0], vec![bx(0., 16., -2., 0., -2., 4., 6., 4.)]),
            part([2.0, 18.0, 4.0], vec![bx(0., 16., -2., 0., -2., 4., 6., 4.)]),
            part([-2.0, 18.0, -4.0], vec![bx(0., 16., -2., 0., -2., 4., 6., 4.)]),
            part([2.0, 18.0, -4.0], vec![bx(0., 16., -2., 0., -2., 4., 6., 4.)]),
        ],
        Pig => vec![
            part([0.0, 12.0, -6.0], vec![bx(0., 0., -4., -4., -8., 8., 8., 8.), bx(16., 16., -2., 0., -9., 4., 3., 1.)]),
            part_rx([0.0, 11.0, 2.0], HALF_PI, vec![bx(28., 8., -5., -10., -7., 10., 16., 8.)]),
            part([-3.0, 18.0, 7.0], vec![bx(0., 16., -2., 0., -2., 4., 6., 4.)]),
            part([3.0, 18.0, 7.0], vec![bx(0., 16., -2., 0., -2., 4., 6., 4.)]),
            part([-3.0, 18.0, -5.0], vec![bx(0., 16., -2., 0., -2., 4., 6., 4.)]),
            part([3.0, 18.0, -5.0], vec![bx(0., 16., -2., 0., -2., 4., 6., 4.)]),
        ],
        Cow => vec![
            part([0.0, 4.0, -8.0], vec![
                bx(0., 0., -4., -4., -6., 8., 8., 6.),
                 bx(22., 0., -5., -5., -4., 1., 3., 1.),
                 bx(22., 0., 4., -5., -4., 1., 3., 1.),
            ]),
            part_rx([0.0, 5.0, 2.0], HALF_PI, vec![bx(18., 4., -6., -10., -7., 12., 18., 10.), bx(52., 0., -2., 2., -8., 4., 6., 1.)]),
            part([-4.0, 12.0, 7.0], vec![bx(0., 16., -2., 0., -2., 4., 12., 4.)]),
            part([4.0, 12.0, 7.0], vec![bx(0., 16., -2., 0., -2., 4., 12., 4.)]),
            part([-4.0, 12.0, -6.0], vec![bx(0., 16., -2., 0., -2., 4., 12., 4.)]),
            part([4.0, 12.0, -6.0], vec![bx(0., 16., -2., 0., -2., 4., 12., 4.)]),
        ],
        Sheep => vec![
            part([0.0, 6.0, -8.0], vec![bx(0., 0., -3., -4., -6., 6., 6., 8.)]),
            part_rx([0.0, 5.0, 2.0], HALF_PI, vec![bx(28., 8., -4., -10., -7., 8., 16., 6.)]),
            part([-3.0, 12.0, 7.0], vec![bx(0., 16., -2., 0., -2., 4., 12., 4.)]),
            part([3.0, 12.0, 7.0], vec![bx(0., 16., -2., 0., -2., 4., 12., 4.)]),
            part([-3.0, 12.0, -5.0], vec![bx(0., 16., -2., 0., -2., 4., 12., 4.)]),
            part([3.0, 12.0, -5.0], vec![bx(0., 16., -2., 0., -2., 4., 12., 4.)]),
            overlay(part([0.0, 6.0, -8.0], vec![bxi(bx(0., 0., -3., -4., -4., 6., 6., 6.), 0.6)])),
            overlay(part_rx([0.0, 5.0, 2.0], HALF_PI, vec![bxi(bx(28., 8., -4., -10., -7., 8., 16., 6.), 1.75)])),
            overlay(part([-3.0, 12.0, 7.0], vec![bxi(bx(0., 16., -2., 0., -2., 4., 6., 4.), 0.5)])),
            overlay(part([3.0, 12.0, 7.0], vec![bxi(bx(0., 16., -2., 0., -2., 4., 6., 4.), 0.5)])),
            overlay(part([-3.0, 12.0, -5.0], vec![bxi(bx(0., 16., -2., 0., -2., 4., 6., 4.), 0.5)])),
            overlay(part([3.0, 12.0, -5.0], vec![bxi(bx(0., 16., -2., 0., -2., 4., 6., 4.), 0.5)])),
        ],
        Chicken => vec![
            part([0.0, 15.0, -4.0], vec![
                bx(0., 0., -2., -6., -2., 4., 6., 3.),
                 bx(14., 0., -2., -4., -4., 4., 2., 2.),
                 bx(14., 4., -1., -2., -3., 2., 2., 2.),
            ]),
            part_rx([0.0, 16.0, 0.0], HALF_PI, vec![bx(0., 9., -3., -4., -3., 6., 8., 6.)]),
            part([-2.0, 19.0, 1.0], vec![bx(26., 0., -1., 0., -3., 3., 5., 3.)]),
            part([1.0, 19.0, 1.0], vec![bx(26., 0., -1., 0., -3., 3., 5., 3.)]),
            part([-4.0, 13.0, 0.0], vec![bx(24., 13., 0., 0., -3., 1., 4., 6.)]),
            part([4.0, 13.0, 0.0], vec![bx(24., 13., -1., 0., -3., 1., 4., 6.)]),
        ],
        Spider => {
            let mut v = vec![
                part([0.0, 15.0, -3.0], vec![bx(32., 4., -4., -4., -8., 8., 8., 8.)]),
                part([0.0, 15.0, 0.0], vec![bx(0., 0., -3., -3., -3., 6., 6., 6.)]),
                part([0.0, 15.0, 9.0], vec![bx(0., 12., -5., -4., -6., 10., 8., 12.)]),
            ];
            let zs = [2.0, 2.0, 1.0, 1.0, 0.0, 0.0, -1.0, -1.0];
            for i in 0..8 {
                let right = i % 2 == 0;
                let b = if right { bx(18., 0., -15., -1., -1., 16., 2., 2.) } else { bx(18., 0., -1., -1., -1., 16., 2., 2.) };
                v.push(part([if right { -4.0 } else { 4.0 }, 15.0, zs[i]], vec![b]));
            }
            v
        }
        Slime => vec![
            part([0.0, 0.0, 0.0], vec![
                bx(0., 16., -3., 17., -3., 6., 6., 6.),
                 bx(32., 0., -3.25, 18., -3.5, 2., 2., 2.),
                 bx(32., 4., 1.25, 18., -3.5, 2., 2., 2.),
                 bx(32., 8., 0., 21., -3.5, 1., 1., 1.),
            ]),
            part([0.0, 0.0, 0.0], vec![bx(0., 0., -4., 16., -4., 8., 8., 8.)]),
        ],
        Ghast => {
            let mut v = vec![part([0.0, 16.0, 0.0], vec![bx(0., 0., -8., -8., -8., 16., 16., 16.)])];
            for i in 0..9 {
                let fx = (((i % 3) as f32 - ((i / 3) % 2) as f32 * 0.5 + 0.25) / 2.0 * 2.0 - 1.0) * 5.0;
                let fz = (((i / 3) as f32 / 2.0) * 2.0 - 1.0) * 5.0;
                let len = 15.0 + ((i * 7) % 7) as f32;
                v.push(part([fx, 24.0, fz], vec![bx(0., 0., -1., 0., -1., 2., len, 2.)]));
            }
            v
        }
        Squid => {
            let mut v = vec![part([0.0, 8.0, 0.0], vec![bx(0., 0., -6., -8., -6., 12., 16., 12.)])];
            for i in 0..8 {
                let a = i as f32 * PI * 2.0 / 8.0;
                let mut p = part([a.cos() * 5.0, 15.0, a.sin() * 5.0], vec![bx(48., 0., -1., 0., -1., 2., 18., 2.)]);
                p.rot = [0.0, i as f32 * PI * -2.0 / 8.0 + HALF_PI, 0.0];
                v.push(p);
            }
            v
        }
        Wolf => vec![
            part([-1.0, 13.5, -7.0], vec![
                bx(0., 0., -3., -3., -2., 6., 6., 4.),
                 bx(16., 14., -3., -5., 0., 2., 2., 1.),
                 bx(16., 14., 1., -5., 0., 2., 2., 1.),
                 bx(0., 10., -1.5, 0., -5., 3., 3., 4.),
            ]),
            part_rx([0.0, 14.0, 2.0], HALF_PI, vec![bx(18., 14., -4., -2., -3., 6., 9., 6.)]),
            part_rx([-1.0, 14.0, 2.0], HALF_PI, vec![bx(21., 0., -4., -3., -3., 8., 6., 7.)]),
            part([-2.5, 16.0, 7.0], vec![bx(0., 18., -1., 0., -1., 2., 8., 2.)]),
            part([0.5, 16.0, 7.0], vec![bx(0., 18., -1., 0., -1., 2., 8., 2.)]),
            part([-2.5, 16.0, -4.0], vec![bx(0., 18., -1., 0., -1., 2., 8., 2.)]),
            part([0.5, 16.0, -4.0], vec![bx(0., 18., -1., 0., -1., 2., 8., 2.)]),
            part([-1.0, 12.0, 8.0], vec![bx(9., 18., -1., 0., -1., 2., 8., 2.)]),
        ],
    }
}

pub struct Pose {
    pub swing: f32,
    pub amt: f32,
    pub head_yaw: f32,
    pub head_pitch: f32,
    pub age: f32,
    pub arms_up: bool,
    pub flap: f32,
}

pub fn pose(kind: MobType, parts: &[PartDef], p: &Pose) -> Vec<[f32; 3]> {
    use MobType::*;
    let mut r: Vec<[f32; 3]> = parts.iter().map(|d| d.rot).collect();
    let s = p.swing * 0.6662;
    let a = p.amt;
    match kind {
        Zombie | Skeleton | PigZombie | Giant => {
            r[0][1] = p.head_yaw; r[0][0] = p.head_pitch;
            r[2][0] = (s + PI).cos() * 2.0 * a * 0.5;
            r[3][0] = s.cos() * 2.0 * a * 0.5;
            if p.arms_up {
                r[2][0] = -HALF_PI * 0.9 + (p.age * 0.067).sin() * 0.05;
                r[3][0] = -HALF_PI * 0.9 - (p.age * 0.067).sin() * 0.05;
            } else {
                r[2][2] += (p.age * 0.09).cos() * 0.05 + 0.05;
                r[3][2] -= (p.age * 0.09).cos() * 0.05 + 0.05;
            }
            r[4][0] = s.cos() * 1.4 * a;
            r[5][0] = (s + PI).cos() * 1.4 * a;
        }
        Creeper => {
            r[0][1] = p.head_yaw; r[0][0] = p.head_pitch;
            r[2][0] = s.cos() * 1.4 * a;
            r[3][0] = (s + PI).cos() * 1.4 * a;
            r[4][0] = (s + PI).cos() * 1.4 * a;
            r[5][0] = s.cos() * 1.4 * a;
        }
        Pig | Cow | Sheep => {
            r[0][1] = p.head_yaw; r[0][0] = p.head_pitch;
            r[2][0] = s.cos() * 1.4 * a;
            r[3][0] = (s + PI).cos() * 1.4 * a;
            r[4][0] = (s + PI).cos() * 1.4 * a;
            r[5][0] = s.cos() * 1.4 * a;
            if kind == Sheep {
                for i in 0..6 { r[6 + i][0] = r[i][0]; r[6 + i][1] = r[i][1]; r[6 + i][2] = r[i][2]; }
            }
        }
        Chicken => {
            r[0][1] = p.head_yaw; r[0][0] = p.head_pitch;
            r[2][0] = s.cos() * 1.4 * a;
            r[3][0] = (s + PI).cos() * 1.4 * a;
            r[4][2] = p.flap;
            r[5][2] = -p.flap;
        }
        Spider => {
            r[0][1] = p.head_yaw; r[0][0] = p.head_pitch;
            let q = PI / 4.0;
            let zb = [-q, q, -q * 0.74, q * 0.74, -q * 0.74, q * 0.74, -q, q];
            let yb = [q, -q, q / 2.0, -q / 2.0, -q / 2.0, q / 2.0, -q, q];
            for i in 0..8 {
                let k = [0.0, PI, PI / 2.0, 3.0 * PI / 2.0][i / 2];
                let sw = -(p.swing * 0.6662 * 2.0 + k).cos() * 0.4 * a;
                let sign = if i % 2 == 0 { 1.0 } else { -1.0 };
                r[3 + i][2] = zb[i];
                r[3 + i][1] = yb[i] + sw * sign;
                r[3 + i][2] += (p.swing * 0.6662 * 2.0 + k).sin().abs() * 0.4 * a * -sign;
            }
        }
        Ghast => {
            for i in 0..9 { r[1 + i][0] = 0.2 * (p.age * 0.3 + i as f32).sin() + 0.1; }
        }
        Squid => {
            for i in 0..8 { r[1 + i][0] = p.flap; }
        }
        Wolf => {
            r[0][1] = p.head_yaw; r[0][0] = p.head_pitch;
            r[3][0] = s.cos() * 1.4 * a;
            r[4][0] = (s + PI).cos() * 1.4 * a;
            r[5][0] = (s + PI).cos() * 1.4 * a;
            r[6][0] = s.cos() * 1.4 * a;
            r[7][0] = 0.63;
            r[7][1] = s.cos() * 0.3 * a;
        }
        Slime => {}
    }
    r
}

pub fn part_matrix(def: &PartDef, rot: [f32; 3]) -> Mat4 {
    Mat4::from_translation(Vec3::new(0.0, 1.5, 0.0))
    * Mat4::from_scale(Vec3::new(1.0 / 16.0, -1.0 / 16.0, -1.0 / 16.0))
    * Mat4::from_translation(Vec3::from(def.point))
    * Mat4::from_rotation_z(rot[2])
    * Mat4::from_rotation_y(rot[1])
    * Mat4::from_rotation_x(rot[0])
}

pub fn part_mesh(def: &PartDef) -> Vec<Vertex> {
    let mut out = Vec::new();
    for b in &def.boxes {
        let (tu, tv) = b.tex;
        let [w, h, d] = b.size;
        let mut x0 = b.from[0] - b.inflate;
        let mut x1 = b.from[0] + w + b.inflate;
        let y0 = b.from[1] - b.inflate;
        let y1 = b.from[1] + h + b.inflate;
        let z0 = b.from[2] - b.inflate;
        let z1 = b.from[2] + d + b.inflate;
        if b.mirror { std::mem::swap(&mut x0, &mut x1); }
        let v: [[f32; 3]; 8] = [
            [x0, y0, z0], [x1, y0, z0], [x1, y1, z0], [x0, y1, z0],
            [x0, y0, z1], [x1, y0, z1], [x1, y1, z1], [x0, y1, z1],
        ];
        let center = Vec3::new((x0 + x1) / 2.0, (y0 + y1) / 2.0, (z0 + z1) / 2.0);
        let quads: [([usize; 4], f32, f32, f32, f32); 6] = [
            ([5, 1, 2, 6], tu + d + w, tv + d, tu + d + w + d, tv + d + h),
            ([0, 4, 7, 3], tu, tv + d, tu + d, tv + d + h),
            ([5, 4, 0, 1], tu + d, tv, tu + d + w, tv + d),
            ([2, 3, 7, 6], tu + d + w, tv, tu + d + w + w, tv + d),
            ([1, 0, 3, 2], tu + d, tv + d, tu + d + w, tv + d + h),
            ([4, 5, 6, 7], tu + d + w + d, tv + d, tu + d + w + d + w, tv + d + h),
        ];
        for (idx, u1, v1, u2, v2) in quads {
            let uv = [(u2, v1), (u1, v1), (u1, v2), (u2, v2)];
            let p: Vec<Vec3> = idx.iter().map(|&i| Vec3::from(v[i])).collect();
            let centroid = (p[0] + p[1] + p[2] + p[3]) * 0.25;
            let n = (p[1] - p[0]).cross(p[2] - p[0]);
            let order: [usize; 6] = if n.dot(centroid - center) >= 0.0 { [0, 1, 2, 0, 2, 3] } else { [0, 2, 1, 0, 3, 2] };
            for &k in &order {
                out.push(Vertex { position: p[k].to_array(), uv: [uv[k].0 / 64.0, 1.0 - uv[k].1 / 32.0] });
            }
        }
    }
    out
}

pub fn skins(kind: MobType, angry: bool, charging: bool) -> [&'static str; 2] {
    use MobType::*;
    match kind {
        Zombie | Giant => ["assets/mob/zombie.png", ""],
        Skeleton => ["assets/mob/skeleton.png", ""],
        PigZombie => ["assets/mob/pigzombie.png", ""],
        Creeper => ["assets/mob/creeper.png", ""],
        Pig => ["assets/mob/pig.png", ""],
        Cow => ["assets/mob/cow.png", ""],
        Sheep => ["assets/mob/sheep.png", "assets/mob/sheep_fur.png"],
        Chicken => ["assets/mob/chicken.png", ""],
        Spider => ["assets/mob/spider.png", ""],
        Slime => ["assets/mob/slime.png", ""],
        Ghast => [if charging { "assets/mob/ghast_fire.png" } else { "assets/mob/ghast.png" }, ""],
        Squid => ["assets/mob/squid.png", ""],
        Wolf => [if angry { "assets/mob/wolf_angry.png" } else { "assets/mob/wolf.png" }, ""],
    }
}
