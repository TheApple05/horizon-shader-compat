#version 120
uniform sampler2D colortex0;
uniform float viewWidth;
uniform float viewHeight;
void main(){
    vec4 scene=texture2D(colortex0,gl_FragCoord.xy/vec2(viewWidth,viewHeight));
    gl_FragColor=vec4(pow(scene.rgb/(1.0+scene.rgb),vec3(1.0/2.2)),scene.a);
}
