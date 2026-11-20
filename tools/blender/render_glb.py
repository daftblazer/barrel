"""Render a character glb (barrel-tool chara-gltf) from the front, side and back into one PNG.

    blender -b -P tools/blender/render_glb.py -- IN.glb OUT.png [--views front,side,back]
            [--frame N] [--size PIXELS] [--only-mesh SUBSTRING] [--each]

Workbench, flat lighting, texture colours: the textures show as they are, unlit. Prints the
joint count, the vertex count and whether every vertex has skin weights. --frame sets the
animation frame (1 is the motion's first key; the glb's keys are 30 a second).
"""

import math
import os
import sys

import bpy
import mathutils
import numpy as np


def parse():
    argv = sys.argv[sys.argv.index("--") + 1:] if "--" in sys.argv else []
    if len(argv) < 2:
        sys.exit(__doc__)
    opts = {"in": argv[0], "out": argv[1], "views": ["front", "side", "back"], "frame": None, "size": 900, "only": None, "each": False}
    i = 2
    while i < len(argv):
        if argv[i] == "--views":
            opts["views"] = argv[i + 1].split(",")
        elif argv[i] == "--frame":
            opts["frame"] = int(argv[i + 1])
        elif argv[i] == "--size":
            opts["size"] = int(argv[i + 1])
        elif argv[i] == "--each":
            opts["each"] = True
            i += 1
            continue
        elif argv[i] == "--only-mesh":
            opts["only"] = argv[i + 1]
        else:
            sys.exit("unknown option " + argv[i])
        i += 2
    return opts


def main():
    o = parse()
    bpy.ops.wm.read_factory_settings(use_empty=True)
    bpy.ops.import_scene.gltf(filepath=o["in"])
    scene = bpy.context.scene
    if o["frame"] is not None:
        scene.frame_set(o["frame"])

    meshes = sorted([ob for ob in scene.objects if ob.type == "MESH" and ob.name.startswith("sub") and (o["only"] is None or o["only"] in ob.name)], key=lambda ob: ob.name)
    armatures = [ob for ob in scene.objects if ob.type == "ARMATURE"]
    for ob in scene.objects:
        if ob.type not in ("MESH",):
            ob.hide_render = True
    for ob in scene.objects:
        if ob.type == "MESH" and ob not in meshes:
            ob.hide_render = True

    # Statistics: joints, vertices, and whether every vertex carries a weight.
    joints = sum(len(a.data.bones) for a in armatures)
    verts = 0
    unweighted = 0
    for ob in meshes:
        verts += len(ob.data.vertices)
        for v in ob.data.vertices:
            if not any(g.weight > 0.0 for g in v.groups):
                unweighted += 1
    print("RESULT joints=%d meshes=%d vertices=%d unweighted_vertices=%d all_weighted=%s" % (joints, len(meshes), verts, unweighted, unweighted == 0))

    # Bounds of the (posed) meshes in world space.
    dg = bpy.context.evaluated_depsgraph_get()
    lo = np.array([1e9] * 3)
    hi = np.array([-1e9] * 3)
    for ob in meshes:
        ev = ob.evaluated_get(dg)
        m = ev.to_mesh()
        n = len(m.vertices)
        if n == 0:
            continue
        co = np.empty(n * 3, dtype=np.float32)
        m.vertices.foreach_get("co", co)
        co = co.reshape(n, 3)
        w = np.array(ev.matrix_world)
        co = co @ w[:3, :3].T + w[:3, 3]
        lo = np.minimum(lo, co.min(axis=0))
        hi = np.maximum(hi, co.max(axis=0))
        ev.to_mesh_clear()
    centre = (lo + hi) / 2
    size = hi - lo
    print("RESULT bounds_min=%s bounds_max=%s" % (np.round(lo, 3).tolist(), np.round(hi, 3).tolist()))

    scene.render.engine = "BLENDER_WORKBENCH"
    sh = scene.display.shading
    sh.light = "FLAT"
    sh.color_type = "TEXTURE"
    sh.show_backface_culling = False
    scene.render.film_transparent = False
    scene.world = bpy.data.worlds.new("w")
    scene.world.color = (0.55, 0.57, 0.6)
    scene.view_settings.view_transform = "Standard"
    scene.render.image_settings.file_format = "PNG"
    scene.render.resolution_x = o["size"]
    scene.render.resolution_y = int(o["size"] * 1.4)

    cam_data = bpy.data.cameras.new("cam")
    cam_data.type = "ORTHO"
    cam_data.ortho_scale = max(size[2] * 1.12, size[0] * 1.12 / (1 / 1.4), 0.1)
    cam = bpy.data.objects.new("cam", cam_data)
    scene.collection.objects.link(cam)
    scene.camera = cam
    # glTF +Z (she faces it) becomes Blender -Y: the front camera looks from -Y.
    views = {"front": (0, -1, 0), "side": (1, 0, 0), "back": (0, 1, 0)}
    tmp = os.path.splitext(o["out"])[0]
    shots = []
    dist = max(size) * 3 + 2
    # --each: the front view of every submesh alone (in order, row by row), to tell the pieces apart.
    jobs = [(name, None) for name in o["views"]]
    if o["each"]:
        scene.render.resolution_y = o["size"]
        jobs = [("front", ob) for ob in meshes]
    for k, (name, only) in enumerate(jobs):
        if only is not None:
            for ob in meshes:
                ob.hide_render = ob is not only
        d = mathutils.Vector(views[name])
        here = centre
        if only is not None:
            # Fit the camera to this submesh alone so small pieces fill their tile.
            m = only.evaluated_get(dg).to_mesh()
            co = np.empty(len(m.vertices) * 3, dtype=np.float32)
            m.vertices.foreach_get("co", co)
            w = np.array(only.matrix_world)
            co = co.reshape(-1, 3) @ w[:3, :3].T + w[:3, 3]
            here = (co.min(axis=0) + co.max(axis=0)) / 2
            cam_data.ortho_scale = max(float((co.max(axis=0) - co.min(axis=0)).max()) * 1.2, 0.05)
        cam.location = mathutils.Vector(here) + d * dist
        cam.rotation_euler = (-d).to_track_quat("-Z", "Y").to_euler()
        path = "%s_%s_%d.png" % (tmp, name, k)
        scene.render.filepath = path
        bpy.ops.render.render(write_still=True)
        shots.append(path)
    columns = 8 if o["each"] else len(shots)

    # One image, the views side by side.
    imgs = [bpy.data.images.load(p) for p in shots]
    w, h = imgs[0].size
    rows = (len(imgs) + columns - 1) // columns
    out = np.zeros((h * rows, w * columns, 4), dtype=np.float32)
    for k, im in enumerate(imgs):
        px = np.empty(w * h * 4, dtype=np.float32)
        im.pixels.foreach_get(px)
        r, c = divmod(k, columns)
        # Blender's pixel rows run bottom to top: the first row of tiles goes at the top.
        out[(rows - 1 - r) * h:(rows - r) * h, c * w:(c + 1) * w] = px.reshape(h, w, 4)
    final = bpy.data.images.new("sheet", w * columns, h * rows)
    final.pixels.foreach_set(out.reshape(-1))
    final.filepath_raw = o["out"]
    final.file_format = "PNG"
    final.save()
    for p in shots:
        os.remove(p)
    print("RESULT wrote %s" % o["out"])


main()
