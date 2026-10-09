struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec3<f32>,
    @location(1) cut: vec4<f32>,
};
@vertex fn vs(@location(0) position: vec3<f32>, @location(1) color: vec3<f32>, @location(2) cut: vec4<f32>) -> VertexOut {
    var out: VertexOut;
    out.position = vec4<f32>(position, 1.0);
    out.color = color;
    out.cut = cut;
    return out;
}
@fragment fn fs(in: VertexOut) -> @location(0) vec4<f32> {
    let radial = dot(in.cut.xy,in.cut.xy);
    let fade = in.cut.z * (1.0-smoothstep(0.45,1.0,radial));
    // Screen-space ordered dither; fixed pixels and real-time hysteresis avoid noise flicker.
    let bayer = (u32(in.position.x)%4u)*4u+(u32(in.position.y)%4u);
    let threshold = f32((bayer*7u)%16u)/16.0;
    if in.cut.w>0.0001 && fade>threshold+0.015 {discard;}
    return vec4<f32>(in.color, 1.0);
}
