# Original compatibility control

Original MIT test shaders. Terrain writes red 0.125. Composite multiplies red by
the GAIN option, reads GL window depth and the worldTime uniform, then final
presents those channels. At terrain depth 0.5 and time 12000, expected RGBA8 is
approximately (32,128,128,255), or (64,128,128,255) with GAIN=2.
