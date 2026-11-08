#version 120
#include "/common.glsl"
/* RENDERTARGETS:0 */
uniform sampler2D colortex0;
uniform sampler2D depthtex0;
uniform int worldTime;
varying vec2 uv;
void main() {
    gl_FragData[0] = vec4(texture2D(colortex0, uv).r * float(GAIN),
        texture2D(depthtex0, uv).r, float(worldTime) / 24000.0, 1.0);
}
