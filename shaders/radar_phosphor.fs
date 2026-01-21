#version 330

in vec2 fragTexCoord;
out vec4 finalColor;

uniform sampler2D texture0;
uniform float intensity = 1.5;

void main()
{
    vec2 uv = fragTexCoord;
    
    // 1. CRT Barrel Distortion
    vec2 centered_uv = uv * 2.0 - 1.0;
    uv = uv + centered_uv * (dot(centered_uv, centered_uv) * 0.04);

    // Hard borders for the tube edge
    if (uv.x < 0.0 || uv.x > 1.0 || uv.y < 0.0 || uv.y > 1.0) {
        finalColor = vec4(0.0, 0.0, 0.0, 1.0);
        return;
    }

    vec4 texel = texture(texture0, uv);

    // 2. Phosphor Color Processing
    // We boost the green and add a "white-hot" highlight to bright areas
    vec3 color = texel.rgb;
    float brightness = dot(color, vec3(0.299, 0.587, 0.114));
    
    // Create an electric mint-green tint
    vec3 phosphorGreen = vec3(0.4, 1.0, 0.5);
    color = mix(color, phosphorGreen * brightness, 0.5);
    
    // Add Bloom/Glow (highlights turn white)
    color += pow(brightness, 3.0) * 0.6;

    // 3. Scanlines (Fixed the undeclared variable error)
    // We use a fixed high number to represent screen density
    float scanline = sin(uv.y * 1200.0) * 0.12;
    color -= scanline;

    // 4. Aperture Grille Mask (Vertical stripes)
    float mask = 0.95 + 0.05 * sin(uv.x * 2000.0);
    color *= mask;

    // 5. Vignette (Dark edges)
    float vignette = 1.0 - dot(centered_uv, centered_uv) * 0.45;
    color *= vignette;

    finalColor = vec4(color * intensity, 1.0);
}
