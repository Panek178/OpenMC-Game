OpenCraft 

The reverse-engineered Minecraft clone written in Rust and OpenGL.

OpenCraft is a lightweight, custom-made Minecraft Beta 1.7.3 client focusing on authentic chunk rendering and custom AABB physics.

To render textures properly, the game requires the original `terrain.png` file from Minecraft Beta 1.7.3.

* The project automatically tries to extract `terrain.png` from your local `.minecraft` folder:
  `~/.minecraft/versions/b1.7.3/b1.7.3.jar` (automatic extract most likely won't work, so doing it manually is recommended)
* If you don't have the `.jar` installed, manually place the `terrain.png` file inside the `src/` directory (`src/terrain.png`).

How to run it?
Make sure you have **Rust** (`cargo`) installed on your system.

Known issues
* **Water Physics / Rendering**: Water blocks are currently basic and may display visual/physics glitches.
* **Mobs & Entities**: Entity system is currently under testing (mobs lack proper textures or AI).
