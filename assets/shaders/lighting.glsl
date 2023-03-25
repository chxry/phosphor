#pragma once
uniform sampler2D shadow_map;
uniform vec3 cam_pos;
uniform vec3 sun_dir;
uniform mat4 sun_view;
uniform mat4 sun_projection;
uniform float specular;
struct light_t {
	vec3 pos;
	vec3 color;
	float strength;
};
uniform light_t lights[100];
uniform int num_lights;

vec3 calc_light(vec3 dir, vec3 color, float atten) {
	vec3 normal = normalize(v_normal);
	dir = normalize(dir);
 	float diffuse = max(dot(normal, dir), 0.0);
	float spec = pow(max(dot(normalize(cam_pos - v_pos), normalize(reflect(dir, normal))), 0.0), 16.0) * specular;
	diffuse *= atten;
	spec *= atten;
	return vec3(diffuse + spec) * color;
}

vec3 lighting() {
	vec3 light = calc_light(sun_dir, vec3(1.0), 1.0);
		
	for (int i = 0; i < num_lights; i++) {
		vec3 dir = lights[i].pos - v_pos;
		float distance = length(dir);
		light += calc_light(dir, lights[i].color, 1.0 / (pow(distance / lights[i].strength, 2.0) + 1.0));
	}

	vec4 light_pos = sun_projection * sun_view * vec4(v_pos, 1.0);
    light_pos = light_pos * 0.5 + 0.5;
    float current = light_pos.z - 0.0015;
    float shadow = 0.0;
    float closest = texture(shadow_map, light_pos.xy).r;
    vec2 texel_size = 1.0 / textureSize(shadow_map, 0);
	int shadow_softness = 2;
    for (int x = -shadow_softness; x <= shadow_softness; ++x) {
      for (int y = -shadow_softness; y <= shadow_softness; ++y) {
        float closest = texture(shadow_map, light_pos.xy + vec2(x, y) * texel_size).r;
        shadow += current > closest ? 1.0 : 0.0;
      }
    }
    shadow /= pow(shadow_softness * 2 + 1, 2);
	light *= (1 - vec3(shadow));
	light += vec3(0.1);

	return light;
}