struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) cut: vec4<f32>,
    @location(2) texture: vec3<f32>,
};
struct Scene { matrix: mat4x4<f32>, player: vec4<f32>, cut: vec4<f32>, look: vec4<f32> };
var<push_constant> scene: Scene;
@vertex fn vs(@location(0) position: vec3<f32>, @location(1) color: vec3<f32>, @location(2) cut: vec4<f32>) -> VertexOut {
    var out: VertexOut;
    out.position = scene.matrix * vec4<f32>(position, 1.0);
    out.position.y = -out.position.y;
    let luminance = dot(color,vec3<f32>(0.2126,0.7152,0.0722));
    out.color = mix(vec3<f32>(luminance),color,scene.look.x);
    out.cut = cut;
    if cut.z >= 0.0 {
        out.cut = vec4<f32>((out.position.x-scene.player.x)/scene.cut.x,(out.position.y+scene.player.y)/scene.cut.y,cut.z*scene.cut.z,scene.player.z-out.position.z);
    }
    if cut.z < -1.5 { out.position = vec4<f32>(position,1.0); out.cut = vec4<f32>(0.0); out.color = color; }
    out.texture = vec3<f32>(cut.xy,select(cut.w,0.0,cut.z<0.0));
    return out;
}
@fragment fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    if in.cut.z < -0.5 {
        let opacity = in.cut.w * (1.0-smoothstep(0.25,1.0,dot(in.cut.xy,in.cut.xy)));
        if opacity < 0.008 { discard; }
        return vec4<f32>(in.color, opacity);
    }
    let radial = dot(in.cut.xy,in.cut.xy);
    let fade = in.cut.z * (1.0-smoothstep(0.45,1.0,radial));
    // Screen-space ordered dither; fixed pixels and real-time hysteresis avoid noise flicker.
    let bayer = (u32(in.position.x)%4u)*4u+(u32(in.position.y)%4u);
    let threshold = f32((bayer*7u)%16u)/16.0;
    if in.cut.w>0.0001 && fade>threshold+0.015 {discard;}
    let haze = smoothstep(0.26, 0.65, in.position.z) * 0.09;
    var shade = 1.0;
    // Each packed row contains eight four-bit texels: true 8x8 nearest pixel tiles.
    let footprint = max(fwidth(in.texture.x),fwidth(in.texture.y))*8.0;
    if in.texture.z > 0.5 {
        let p = vec2<u32>(floor(fract(in.texture.xy)*8.0));
        let tile = min(u32(in.texture.z)-1u,7u);
        let row = TEXTURE_ROWS[tile*8u+p.y];
        let pixel = (row>>(p.x*4u))&15u;
        let detail = 0.65+f32(pixel)*0.04;
        shade = mix(detail,1.0,smoothstep(0.9,2.5,footprint));
    }
    return vec4<f32>(mix(in.color*shade, vec3<f32>(0.40,0.48,0.55), haze), 1.0);
}
