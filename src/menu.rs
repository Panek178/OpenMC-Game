// menu.rs - menu główne OpenMC
// - animowane logo 3D "OpenMC" złożone z bloków cobblestone (spadają i układają się, potem lekko się kołysze)
// - przyciski w stylu Minecrafta (obramowanie, fazowanie, podświetlenie po najechaniu)
// - wbudowana czcionka bitmapowa 5x7 (nie wymaga żadnych plików)
// - własny shader i VAO/VBO, więc main.rs nie musi nic przygotowywać

use bytemuck::{Pod, Zeroable};
use glam::{Mat4, Vec3};
use glow::HasContext;
use std::time::Instant;

const GUI_H: f32 = 240.0; // wirtualna wysokość GUI (szerokość wynika z proporcji okna)
const LOGO_W: f32 = 35.0; // szerokość logo w blokach (9 liter * 6 - 1)
const COBBLE_TILE: i32 = 16; // kafelek cobblestone w terrain.png
const DIRT_TILE: i32 = 2; // kafelek ziemi w terrain.png

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum GameState {
    MainMenu,
    Playing,
    Options,
}

#[derive(PartialEq, Eq, Clone, Copy, Debug)]
pub enum MenuAction {
    None,
    Play,
    Quit,
}

#[derive(PartialEq, Eq, Clone, Copy)]
enum ButtonId {
    Singleplayer,
    Options,
    Quit,
    RenderDistance,
    Back,
}

struct Button {
    id: ButtonId,
    x: f32,
    y: f32,
    w: f32,
    h: f32,
    label: String,
}

#[repr(C)]
#[derive(Copy, Clone, Pod, Zeroable)]
struct MenuVertex {
    pos: [f32; 3],
    uv: [f32; 2],
    color: [f32; 4],
}

struct LogoBlock {
    x: f32,
    y: f32,
    col: i32,
    row: i32,
}

struct Glyph {
    top: usize,
    rows: &'static [&'static str],
}

// Litery logo "OpenMC" (5 kolumn szerokości, kolejne wiersze od góry; 'top' = wiersz startowy)
const LOGO_GLYPHS: [Glyph; 6] = [
    // O
    Glyph { top: 0, rows: &[".###.", "#...#", "#...#", "#...#", "#...#", ".###."] },
// p
Glyph { top: 1, rows: &["####.", "#...#", "#...#", "#...#", "####.", "#....", "#...."] },
// e
Glyph { top: 1, rows: &[".###.", "#...#", "#####", "#....", ".####"] },
// n
Glyph { top: 1, rows: &["####.", "#...#", "#...#", "#...#", "#...#"] },
// M
Glyph { top: 0, rows: &["#...#", "##.##", "#.#.#", "#...#", "#...#", "#...#"] },
// C
Glyph { top: 0, rows: &[".####", "#....", "#....", "#....", "#....", ".####"] },
];

// Ściany sześcianu: 4 rogi (lewy-dół, prawy-dół, prawy-góra, lewy-góra patrząc z zewnątrz) + normalna
const CUBE_FACES: [([[f32; 3]; 4], [f32; 3]); 6] = [
    ([[-0.5, -0.5, 0.5], [0.5, -0.5, 0.5], [0.5, 0.5, 0.5], [-0.5, 0.5, 0.5]], [0.0, 0.0, 1.0]),
    ([[0.5, -0.5, -0.5], [-0.5, -0.5, -0.5], [-0.5, 0.5, -0.5], [0.5, 0.5, -0.5]], [0.0, 0.0, -1.0]),
    ([[0.5, -0.5, 0.5], [0.5, -0.5, -0.5], [0.5, 0.5, -0.5], [0.5, 0.5, 0.5]], [1.0, 0.0, 0.0]),
    ([[-0.5, -0.5, -0.5], [-0.5, -0.5, 0.5], [-0.5, 0.5, 0.5], [-0.5, 0.5, -0.5]], [-1.0, 0.0, 0.0]),
    ([[-0.5, 0.5, 0.5], [0.5, 0.5, 0.5], [0.5, 0.5, -0.5], [-0.5, 0.5, -0.5]], [0.0, 1.0, 0.0]),
    ([[-0.5, -0.5, -0.5], [0.5, -0.5, -0.5], [0.5, -0.5, 0.5], [-0.5, -0.5, 0.5]], [0.0, -1.0, 0.0]),
];

const VS_SRC: &str = r#"#version 330 core
layout (location = 0) in vec3 aPos;
layout (location = 1) in vec2 aUV;
layout (location = 2) in vec4 aColor;
out vec2 vUV;
out vec4 vColor;
uniform mat4 uMVP;
void main() {
gl_Position = uMVP * vec4(aPos, 1.0);
vUV = aUV;
vColor = aColor;
}"#;

// uv.x < 0 oznacza "kolor jednolity" (przyciski, tekst), w przeciwnym razie tekstura * kolor
const FS_SRC: &str = r#"#version 330 core
in vec2 vUV;
in vec4 vColor;
out vec4 FragColor;
uniform sampler2D uTexture;
void main() {
if (vUV.x < 0.0) {
    FragColor = vColor;
    } else {
        vec4 t = texture(uTexture, vUV);
        if (t.a < 0.1) discard;
        FragColor = vec4(t.rgb * vColor.rgb, t.a * vColor.a);
        }
        }"#;

        /// Kompiluje i linkuje program shaderowy, w razie błędu wypisuje log OpenGL.
        pub unsafe fn compile_program(gl: &glow::Context, vs_src: &str, fs_src: &str) -> glow::Program {
            let program = gl.create_program().expect("create_program");
            let mut shaders = Vec::new();
            for (ty, src) in [(glow::VERTEX_SHADER, vs_src), (glow::FRAGMENT_SHADER, fs_src)] {
                let s = gl.create_shader(ty).expect("create_shader");
                gl.shader_source(s, src);
                gl.compile_shader(s);
                if !gl.get_shader_compile_status(s) {
                    panic!("Błąd kompilacji shadera: {}", gl.get_shader_info_log(s));
                }
                gl.attach_shader(program, s);
                shaders.push(s);
            }
            gl.link_program(program);
            if !gl.get_program_link_status(program) {
                panic!("Błąd linkowania programu: {}", gl.get_program_info_log(program));
            }
            for s in shaders {
                gl.detach_shader(program, s);
                gl.delete_shader(s);
            }
            program
        }

        fn hash01(a: i32, b: i32) -> f32 {
            let mut h = (a as u32).wrapping_mul(374_761_393) ^ (b as u32).wrapping_mul(668_265_263);
            h = (h ^ (h >> 13)).wrapping_mul(1_274_126_177);
            ((h ^ (h >> 16)) & 0xFFFF) as f32 / 65535.0
        }

        fn tile_uv(tile: i32) -> [f32; 4] {
            let u0 = (tile % 16) as f32 / 16.0;
            let v0 = (15 - tile / 16) as f32 / 16.0;
            [u0, v0, u0 + 1.0 / 16.0, v0 + 1.0 / 16.0]
        }

        // Czcionka 5x7 (wielkie litery, cyfry, kilka znaków). Bit 4 = lewa kolumna.
        fn glyph(c: char) -> [u8; 7] {
            match c {
                'A' => [0b01110, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
                'B' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10001, 0b10001, 0b11110],
                'C' => [0b01110, 0b10001, 0b10000, 0b10000, 0b10000, 0b10001, 0b01110],
                'D' => [0b11110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b11110],
                'E' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b11111],
                'F' => [0b11111, 0b10000, 0b10000, 0b11110, 0b10000, 0b10000, 0b10000],
                'G' => [0b01110, 0b10001, 0b10000, 0b10111, 0b10001, 0b10001, 0b01111],
                'H' => [0b10001, 0b10001, 0b10001, 0b11111, 0b10001, 0b10001, 0b10001],
                'I' => [0b01110, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
                'J' => [0b00111, 0b00010, 0b00010, 0b00010, 0b00010, 0b10010, 0b01100],
                'K' => [0b10001, 0b10010, 0b10100, 0b11000, 0b10100, 0b10010, 0b10001],
                'L' => [0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b10000, 0b11111],
                'M' => [0b10001, 0b11011, 0b10101, 0b10101, 0b10001, 0b10001, 0b10001],
                'N' => [0b10001, 0b11001, 0b10101, 0b10011, 0b10001, 0b10001, 0b10001],
                'O' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
                'P' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10000, 0b10000, 0b10000],
                'Q' => [0b01110, 0b10001, 0b10001, 0b10001, 0b10101, 0b10010, 0b01101],
                'R' => [0b11110, 0b10001, 0b10001, 0b11110, 0b10100, 0b10010, 0b10001],
                'S' => [0b01111, 0b10000, 0b10000, 0b01110, 0b00001, 0b00001, 0b11110],
                'T' => [0b11111, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0b00100],
                'U' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01110],
                'V' => [0b10001, 0b10001, 0b10001, 0b10001, 0b10001, 0b01010, 0b00100],
                'W' => [0b10001, 0b10001, 0b10001, 0b10101, 0b10101, 0b11011, 0b10001],
                'X' => [0b10001, 0b10001, 0b01010, 0b00100, 0b01010, 0b10001, 0b10001],
                'Y' => [0b10001, 0b10001, 0b01010, 0b00100, 0b00100, 0b00100, 0b00100],
                'Z' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b10000, 0b11111],
                '0' => [0b01110, 0b10001, 0b10011, 0b10101, 0b11001, 0b10001, 0b01110],
                '1' => [0b00100, 0b01100, 0b00100, 0b00100, 0b00100, 0b00100, 0b01110],
                '2' => [0b01110, 0b10001, 0b00001, 0b00010, 0b00100, 0b01000, 0b11111],
                '3' => [0b11110, 0b00001, 0b00001, 0b01110, 0b00001, 0b00001, 0b11110],
                '4' => [0b00010, 0b00110, 0b01010, 0b10010, 0b11111, 0b00010, 0b00010],
                '5' => [0b11111, 0b10000, 0b11110, 0b00001, 0b00001, 0b10001, 0b01110],
                '6' => [0b00110, 0b01000, 0b10000, 0b11110, 0b10001, 0b10001, 0b01110],
                '7' => [0b11111, 0b00001, 0b00010, 0b00100, 0b01000, 0b01000, 0b01000],
                '8' => [0b01110, 0b10001, 0b10001, 0b01110, 0b10001, 0b10001, 0b01110],
                '9' => [0b01110, 0b10001, 0b10001, 0b01111, 0b00001, 0b00010, 0b01100],
                '.' => [0, 0, 0, 0, 0, 0b01100, 0b01100],
                '-' => [0, 0, 0, 0b11111, 0, 0, 0],
                ':' => [0, 0b01100, 0b01100, 0, 0b01100, 0b01100, 0],
                '!' => [0b00100, 0b00100, 0b00100, 0b00100, 0b00100, 0, 0b00100],
                _ => [0; 7],
            }
        }

        fn rect(out: &mut Vec<MenuVertex>, x0: f32, y0: f32, x1: f32, y1: f32, color: [f32; 4]) {
            let uv = [-1.0, -1.0];
            for p in [[x0, y0], [x1, y0], [x1, y1], [x1, y1], [x0, y1], [x0, y0]] {
                out.push(MenuVertex { pos: [p[0], p[1], 0.0], uv, color });
            }
        }

        fn quad_uv(out: &mut Vec<MenuVertex>, x0: f32, y0: f32, x1: f32, y1: f32, uv: [f32; 4], color: [f32; 4]) {
            let pts = [([x0, y0], [uv[0], uv[1]]), ([x1, y0], [uv[2], uv[1]]), ([x1, y1], [uv[2], uv[3]]), ([x0, y1], [uv[0], uv[3]])];
            for i in [0usize, 1, 2, 2, 3, 0] {
                out.push(MenuVertex { pos: [pts[i].0[0], pts[i].0[1], 0.0], uv: pts[i].1, color });
            }
        }

        fn text_width(s: &str, u: f32) -> f32 {
            let n = s.chars().count() as f32;
            if n == 0.0 { 0.0 } else { n * 6.0 * u - u }
        }

        fn draw_text(out: &mut Vec<MenuVertex>, x: f32, y: f32, s: &str, u: f32, color: [f32; 4]) {
            let mut cx = x;
            for ch in s.chars() {
                let g = glyph(ch.to_ascii_uppercase());
                for (r, bits) in g.iter().enumerate() {
                    let y1 = y + (7 - r) as f32 * u;
                    let y0 = y1 - u;
                    let mut c = 0usize;
                    while c < 5 {
                        let bit_shift = 0x10u8 >> c;
                        if bits & bit_shift != 0 {
                            let start = c;
                            while c < 5 && (bits & (0x10u8 >> c)) != 0 {
                                c += 1;
                            }
                            rect(out, cx + start as f32 * u, y0, cx + c as f32 * u, y1, color);
                        } else {
                            c += 1;
                        }
                    }
                }
                cx += 6.0 * u;
            }
        }

        fn draw_text_shadow(out: &mut Vec<MenuVertex>, x: f32, y: f32, s: &str, u: f32, color: [f32; 4]) {
            draw_text(out, x + u, y - u, s, u, [0.12, 0.12, 0.12, 1.0]);
            draw_text(out, x, y, s, u, color);
        }

        fn draw_text_centered(out: &mut Vec<MenuVertex>, cx: f32, cy: f32, s: &str, u: f32, color: [f32; 4]) {
            let w = text_width(s, u);
            draw_text_shadow(out, cx - w / 2.0, cy - 3.5 * u, s, u, color);
        }

        fn draw_button(out: &mut Vec<MenuVertex>, b: &Button, hovered: bool) {
            let (fill, hl, sh, text) = if hovered {
                ([0.36, 0.42, 0.66, 1.0], [0.62, 0.68, 0.95, 1.0], [0.18, 0.21, 0.40, 1.0], [1.0, 1.0, 0.63, 1.0])
            } else {
                ([0.42, 0.42, 0.42, 1.0], [0.70, 0.70, 0.70, 1.0], [0.23, 0.23, 0.23, 1.0], [0.90, 0.90, 0.90, 1.0])
            };
            let (x, y, w, h) = (b.x, b.y, b.w, b.h);
            rect(out, x, y, x + w, y + h, [0.0, 0.0, 0.0, 1.0]); // czarna ramka
            rect(out, x + 1.0, y + 1.0, x + w - 1.0, y + h - 1.0, fill);
            rect(out, x + 1.0, y + h - 2.0, x + w - 1.0, y + h - 1.0, hl); // góra
            rect(out, x + 1.0, y + 1.0, x + 2.0, y + h - 1.0, hl); // lewo
            rect(out, x + 1.0, y + 1.0, x + w - 1.0, y + 2.0, sh); // dół
            rect(out, x + w - 2.0, y + 1.0, x + w - 1.0, y + h - 1.0, sh); // prawo
            draw_text_centered(out, x + w / 2.0, y + h / 2.0, &b.label, 1.3, text);
        }

        fn push_cube(out: &mut Vec<MenuVertex>, m: &Mat4, uv: [f32; 4]) {
            let light = Vec3::new(0.35, 0.8, 0.5).normalize();
            let uvs = [[uv[0], uv[1]], [uv[2], uv[1]], [uv[2], uv[3]], [uv[0], uv[3]]];
            for (corners, n) in CUBE_FACES.iter() {
                let nw = m.transform_vector3(Vec3::from(*n)).normalize();
                let b = 0.42 + 0.58 * nw.dot(light).max(0.0);
                let color = [b, b, b, 1.0];
                let mut pts = [[0.0f32; 3]; 4];
                for i in 0..4 {
                    pts[i] = m.transform_point3(Vec3::from(corners[i])).to_array();
                }
                for i in [0usize, 1, 2, 2, 3, 0] {
                    out.push(MenuVertex { pos: pts[i], uv: uvs[i], color });
                }
            }
        }

        fn build_logo_blocks() -> Vec<LogoBlock> {
            let mut v = Vec::new();
            let mut col0 = 0;
            for g in LOGO_GLYPHS.iter() {
                for (ri, row) in g.rows.iter().enumerate() {
                    for (ci, ch) in row.chars().enumerate() {
                        if ch == '#' {
                            let col = col0 + ci as i32;
                            let r = g.top as i32 + ri as i32;
                            v.push(LogoBlock {
                                x: col as f32 + 0.5 - LOGO_W / 2.0,
                                y: 4.0 - (r as f32 + 0.5),
                                   col,
                                   row: r,
                            });
                        }
                    }
                }
                col0 += 6;
            }
            v
        }

        pub struct MainMenuManager {
            pub state: GameState,
            pub render_distance: i32,
            program: glow::Program,
            vao: glow::VertexArray,
            vbo: glow::Buffer,
            u_mvp: Option<glow::UniformLocation>,
            u_tex: Option<glow::UniformLocation>,
            mouse_px: (f32, f32),
            win: (f32, f32),
            start: Instant,
            logo_blocks: Vec<LogoBlock>,
        }

        impl MainMenuManager {
            pub fn new(gl: &glow::Context) -> Self {
                unsafe {
                    let program = compile_program(gl, VS_SRC, FS_SRC);
                    let vao = gl.create_vertex_array().expect("vao");
                    let vbo = gl.create_buffer().expect("vbo");
                    gl.bind_vertex_array(Some(vao));
                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(vbo));
                    let stride = std::mem::size_of::<MenuVertex>() as i32;
                    gl.vertex_attrib_pointer_f32(0, 3, glow::FLOAT, false, stride, 0);
                    gl.enable_vertex_attrib_array(0);
                    gl.vertex_attrib_pointer_f32(1, 2, glow::FLOAT, false, stride, 12);
                    gl.enable_vertex_attrib_array(1);
                    gl.vertex_attrib_pointer_f32(2, 4, glow::FLOAT, false, stride, 20);
                    gl.enable_vertex_attrib_array(2);
                    gl.bind_vertex_array(None);
                    let u_mvp = gl.get_uniform_location(program, "uMVP");
                    let u_tex = gl.get_uniform_location(program, "uTexture");
                    Self {
                        state: GameState::MainMenu,
                        render_distance: 3,
                        program,
                        vao,
                        vbo,
                        u_mvp,
                        u_tex,
                        mouse_px: (0.0, 0.0),
                        win: (800.0, 600.0),
                        start: Instant::now(),
                        logo_blocks: build_logo_blocks(),
                    }
                }
            }

            pub fn reset_animation(&mut self) {
                self.start = Instant::now();
            }

            /// Powrót do głównego menu (np. po wyjściu z gry) - logo układa się od nowa.
            pub fn show_main_menu(&mut self) {
                self.state = GameState::MainMenu;
                self.reset_animation();
            }

            /// Escape na ekranie opcji.
            pub fn back(&mut self) {
                if self.state == GameState::Options {
                    self.show_main_menu();
                }
            }

            pub fn update_cursor(&mut self, x: f64, y: f64, width: u32, height: u32) {
                self.mouse_px = (x as f32, y as f32);
                self.win = (width.max(1) as f32, height.max(1) as f32);
            }

            fn gui_width(&self) -> f32 {
                GUI_H * self.win.0 / self.win.1
            }

            fn mouse_gui(&self) -> (f32, f32) {
                (
                    self.mouse_px.0 / self.win.0 * self.gui_width(),
                 GUI_H - self.mouse_px.1 / self.win.1 * GUI_H,
                )
            }

            fn buttons(&self) -> Vec<Button> {
                let gw = self.gui_width();
                let bw = 200.0f32.min(gw - 20.0);
                let bx = (gw - bw) / 2.0;
                let mk = |id: ButtonId, cy: f32, label: &str| Button { id, x: bx, y: cy - 10.0, w: bw, h: 20.0, label: label.to_string() };
                match self.state {
                    GameState::MainMenu => vec![
                        mk(ButtonId::Singleplayer, 112.0, "Singleplayer"),
                        mk(ButtonId::Options, 88.0, "Options"),
                        mk(ButtonId::Quit, 64.0, "Quit Game"),
                    ],
                    GameState::Options => vec![
                        mk(ButtonId::RenderDistance, 130.0, &format!("Render distance: {}", self.render_distance)),
                        mk(ButtonId::Back, 40.0, "Back"),
                    ],
                    GameState::Playing => vec![],
                }
            }

            fn hovered(&self) -> Option<ButtonId> {
                let (mx, my) = self.mouse_gui();
                self.buttons()
                .iter()
                .find(|b| mx >= b.x && mx <= b.x + b.w && my >= b.y && my <= b.y + b.h)
                .map(|b| b.id)
            }

            /// Obsługa kliknięcia lewym przyciskiem w menu.
            pub fn click(&mut self) -> MenuAction {
                match self.hovered() {
                    Some(ButtonId::Singleplayer) => {
                        self.state = GameState::Playing;
                        MenuAction::Play
                    }
                    Some(ButtonId::Options) => {
                        self.state = GameState::Options;
                        MenuAction::None
                    }
                    Some(ButtonId::Quit) => MenuAction::Quit,
                    Some(ButtonId::RenderDistance) => {
                        self.render_distance = if self.render_distance >= 8 { 2 } else { self.render_distance + 1 };
                        MenuAction::None
                    }
                    Some(ButtonId::Back) => {
                        self.show_main_menu();
                        MenuAction::None
                    }
                    None => MenuAction::None,
                }
            }

            fn build_background(&self, out: &mut Vec<MenuVertex>, gui_w: f32) {
                let tile = 32.0;
                let nx = (gui_w / tile).ceil() as i32;
                let ny = (GUI_H / tile).ceil() as i32;
                let uv = tile_uv(DIRT_TILE);
                for ix in 0..nx {
                    for iy in 0..ny {
                        let x0 = ix as f32 * tile;
                        let y0 = iy as f32 * tile;
                        quad_uv(out, x0, y0, x0 + tile, y0 + tile, uv, [0.35, 0.35, 0.35, 1.0]);
                    }
                }
            }

            fn build_logo(&self, out: &mut Vec<MenuVertex>, t: f32) {
                let uv = tile_uv(COBBLE_TILE);
                let sway = (t * 0.7).sin() * 0.22;
                let bob = (t * 1.3).sin() * 0.4;
                let global = Mat4::from_translation(Vec3::new(0.0, bob, 0.0))
                * Mat4::from_rotation_x(0.30)
                * Mat4::from_rotation_y(sway);

                for b in &self.logo_blocks {
                    let h1 = hash01(b.col, b.row);
                    let h2 = hash01(b.row + 101, b.col + 57);
                    let delay = b.col as f32 * 0.035 + h1 * 0.6;
                    let local = t - delay;
                    if local < 0.0 {
                        continue;
                    }
                    let dur = 0.9;
                    let p = (local / dur).min(1.0);
                    let fall = (1.0 - p * p) * (40.0 + h2 * 20.0);
                    let q = local - dur;
                    let bounce = if q > 0.0 { 0.35 * (-5.0 * q).exp() * (q * 14.0).sin().abs() } else { 0.0 };
                    let spin = (1.0 - p) * (h1 * 6.0 - 3.0);
                    let m = global
                    * Mat4::from_translation(Vec3::new(b.x, b.y + fall + bounce, (1.0 - p) * (h2 * 8.0 - 4.0)))
                    * Mat4::from_rotation_y(spin)
                    * Mat4::from_rotation_x(spin * 0.7);
                    push_cube(out, &m, uv);
                }
            }

            fn build_ui(&self, out: &mut Vec<MenuVertex>, gui_w: f32) {
                let hovered = self.hovered();
                for b in self.buttons() {
                    draw_button(out, &b, hovered == Some(b.id));
                }
                match self.state {
                    GameState::MainMenu => {
                        draw_text_shadow(out, 3.0, 3.0, "OpenMC Beta 1.7.3", 1.0, [0.85, 0.85, 0.85, 1.0]);
                        let s = "By Panek";
                        draw_text_shadow(out, gui_w - text_width(s, 1.0) - 4.0, 3.0, s, 1.0, [0.85, 0.85, 0.85, 1.0]);
                    }
                    GameState::Options => {
                        draw_text_centered(out, gui_w / 2.0, 205.0, "Options", 1.8, [1.0, 1.0, 1.0, 1.0]);
                    }
                    GameState::Playing => {}
                }
            }

            pub fn render(&mut self, gl: &glow::Context, texture: glow::Texture, width: u32, height: u32) {
                self.win = (width.max(1) as f32, height.max(1) as f32);
                let gui_w = self.gui_width();
                let t = self.start.elapsed().as_secs_f32();

                let mut bg = Vec::new();
                let mut logo = Vec::new();
                let mut ui = Vec::new();
                self.build_background(&mut bg, gui_w);
                if self.state == GameState::MainMenu {
                    self.build_logo(&mut logo, t);
                }
                self.build_ui(&mut ui, gui_w);

                let bg_n = bg.len() as i32;
                let logo_n = logo.len() as i32;
                let ui_n = ui.len() as i32;
                let mut all = bg;
                all.extend(logo);
                all.extend(ui);

                let aspect = self.win.0 / self.win.1;
                let fov = 45.0f32.to_radians();
                let t_half = (fov / 2.0).tan();
                let dist = (LOGO_W / 2.0 / (0.66 * t_half * aspect)).max(55.0);
                let proj = Mat4::from_translation(Vec3::new(0.0, 0.5, 0.0)) * Mat4::perspective_rh_gl(fov, aspect, 1.0, 400.0);
                let view = Mat4::look_at_rh(Vec3::new(0.0, 0.0, dist), Vec3::ZERO, Vec3::Y);
                let logo_mvp = (proj * view).to_cols_array();
                let ortho = Mat4::orthographic_rh_gl(0.0, gui_w, 0.0, GUI_H, -10.0, 10.0).to_cols_array();

                unsafe {
                    gl.disable(glow::DEPTH_TEST);
                    gl.disable(glow::CULL_FACE);
                    gl.use_program(Some(self.program));
                    gl.active_texture(glow::TEXTURE0);
                    gl.bind_texture(glow::TEXTURE_2D, Some(texture));
                    gl.uniform_1_i32(self.u_tex.as_ref(), 0);

                    gl.bind_vertex_array(Some(self.vao));
                    gl.bind_buffer(glow::ARRAY_BUFFER, Some(self.vbo));
                    gl.buffer_data_u8_slice(glow::ARRAY_BUFFER, bytemuck::cast_slice(&all), glow::DYNAMIC_DRAW);

                    // 1. tło
                    gl.uniform_matrix_4_f32_slice(self.u_mvp.as_ref(), false, &ortho);
                    gl.draw_arrays(glow::TRIANGLES, 0, bg_n);

                    // 2. logo 3D (z buforem głębi)
                    if logo_n > 0 {
                        gl.clear(glow::DEPTH_BUFFER_BIT);
                        gl.enable(glow::DEPTH_TEST);
                        gl.uniform_matrix_4_f32_slice(self.u_mvp.as_ref(), false, &logo_mvp);
                        gl.draw_arrays(glow::TRIANGLES, bg_n, logo_n);
                        gl.disable(glow::DEPTH_TEST);
                    }

                    // 3. przyciski i tekst
                    gl.uniform_matrix_4_f32_slice(self.u_mvp.as_ref(), false, &ortho);
                    gl.draw_arrays(glow::TRIANGLES, bg_n + logo_n, ui_n);

                    gl.bind_vertex_array(None);
                    gl.enable(glow::DEPTH_TEST);
                    gl.enable(glow::CULL_FACE);
                }
            }
        }
