# Renderer

`crates/render` draws the games' data with Vulkan 1.1 through `ash`. For now it is a single
tool, `barrel-view`, that renders one model offscreen and writes the frame as PNG.

`barrel-view --field FIELD_DIR out.png` draws a field's objects: every instance in the
field's placement list, with its model from the `_Obj` container and the field's shared
texture pack (the object models carry no textures of their own). Each instance is scaled,
rotated (x, then y, then z, in radians) and moved on the CPU. AREA_01_01 places 112
objects (126 draws, 52,089 triangles), and they come out where they belong: a ring of
cliffs around a valley of trees, rocks and flowers. Under them it draws the
terrain from the field's `.ktf`: the heightmap as a grid, shaded by `terrain.frag`, which
reads the two blend maps across the whole field (UV 0..1) and uses their six channels
(map 0 RGB, map 1 RGB) as weights for the first six ground layers, each tiled at the scale
its layer record gives (AREA_01_01: 300, 300, 400, 300, 200 and 100 units a repeat). In
AREA_01_01 that gives sand on the open ground and dirt and grass in the valley under the
trees. Still to check against the game: the blend maps' orientation, and fields with more
than six layers, where the settings block must say which layers a map's channels weight.

Options: `--size N` (64..1280), `--yaw DEG`, `--pitch DEG` (45 by default for fields),
`--no-alpha` to turn off the alpha test. `BARREL_DRY=1` builds the scene and prints its
bounds without touching the GPU.

What it does today:
- uploads the model's submeshes (positions, normals, UVs from `g1m::Geometry::simple_mesh`)
  and the texture pack's BC1/BC3 textures as they are, with all mip levels;
- picks each submesh's texture from its material (the first texture of type 1, else the
  first texture);
- draws with depth testing, no culling, and an alpha test at 0.3 for cut-out textures
  (hair, lace);
- places a perspective camera on the model's bounds, in front of it.

Conventions learned from the data:
- The games are DirectX, left-handed. The renderer mirrors x on the vertices as it builds
  the scene (not in the projection, so the camera is placed on the mirrored scene), and
  models face +z.
- Texture rows are top to bottom, as Vulkan expects; UVs are used as they are.

Not done yet: skinning (bones are read, see `formats.md`), the games' own shaders and
lighting (toon shading, rim light), materials beyond the first texture, a window and
interactive camera, and fields.

Every run is one frame and exits; set `BARREL_VALIDATION=1` to run with the Khronos
validation layer, which Escha's model and an item pass without errors.
