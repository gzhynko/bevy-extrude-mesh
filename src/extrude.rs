use bevy::prelude::*;
use bevy::render::mesh::{Indices, PrimitiveTopology};
use bevy::render::render_asset::RenderAssetUsages;
use crate::bezier::OrientedPoint;

pub struct ExtrudeShape {
    vertices: Vec<[f32; 3]>,
    normals: Vec<[f32; 3]>,
    face_indices: Vec<u32>,
    edges: Vec<u32>,
    u_coords: Vec<f32>,
}

impl ExtrudeShape {
    pub fn from_mesh(mesh: &Mesh) -> Self {
        // Vertices
        let vertices = mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap().as_float3().unwrap().to_vec();

        // Indices
        let indices = mesh.indices().unwrap();
        let index_array;
        match indices {
            Indices::U16(i) => index_array = i.iter().map(|x| *x as u32).collect::<Vec<u32>>().clone(),
            Indices::U32(i) => index_array = i.clone(),
        }
        let mut edges = Vec::new();
        for i in (0..index_array.len()).step_by(3) {
            // triangle = 3 edges
            let edge1 = (index_array[i], index_array[i+1]);
            edges.push(edge1);
            let edge2 = (index_array[i+1], index_array[i+2]);
            edges.push(edge2);
            let edge3 = (index_array[i+2], index_array[i]);
            edges.push(edge3);
        }

        // A messy way to remove the unneeded edges of the triangles (the ones in the "center")
        let edges_clone = edges.clone();
        let mut removed = Vec::new();
        edges.retain(|edge| {
            if removed.contains(&(edge.1, edge.0)) || edges_clone.contains(&(edge.1, edge.0)) {
                removed.push(*edge);
                false
            } else {
                true
            }
        });
        let edges_array: Vec<u32> = edges.iter().flat_map(|edge| [edge.0, edge.1]).collect();

        // UVs
        let uvs_result = mesh.attribute(Mesh::ATTRIBUTE_UV_0);
        let mut u_coords = Vec::new();
        if let Some(uvs) = uvs_result {
            if let Some(uv_array) = uvs.as_float3() {
                u_coords = uv_array.iter().map(|uv| uv[0]).collect();
            }
        }

        // Normals
        // Here we ignore the mesh normals and instead calculate normals that
        let vertex_count = vertices.len();
        let mut edge_normals = vec![[0., 0., 0.]; vertex_count];
        for i in 0..vertex_count {
            let j = (i + 1) % vertex_count;
            let edge_vec = Vec3::from_array(vertices[j]) - Vec3::from_array(vertices[i]);
            edge_normals[i] = Vec3::new(-edge_vec.y, edge_vec.x, edge_vec.z).normalize().to_array();
        }

        let mut vertex_normals = vec![[0., 0., 0.]; vertex_count];
        for i in 0..vertex_count {
            let j = (vertex_count + i - 1) % vertex_count;
            vertex_normals[i] = (Vec3::from_array(edge_normals[i]) + Vec3::from_array(edge_normals[j])).normalize().to_array();
        }

        Self {
            vertices,
            normals: vertex_normals,
            face_indices: index_array,
            edges: edges_array,
            u_coords,
        }
    }
}

pub fn extrude(shape: &ExtrudeShape, path: &Vec<OrientedPoint>) -> Mesh {
    let shape_vertex_count = shape.vertices.len();
    let segments = path.len() - 1;
    let edge_loops = path.len();
    let vertex_count = shape_vertex_count * edge_loops;
    let tri_count = shape.edges.len() * segments + 2 * shape.face_indices.len();
    let index_count = tri_count * 3;

    //println!("extrude path (oriented points): {:?}", path);

    let mut mesh_vertices = vec![[0.,0.,0.]; vertex_count];
    let mut mesh_indices: Vec<u32> = vec![0u32; index_count];
    let mut mesh_normals: Vec<[f32; 3]> = vec![[0.,0.,0.]; vertex_count];
    let mut mesh_uvs: Vec<[f32; 2]> = vec![[0.,0.]; vertex_count];

    // Vertices + normals + UVs
    for (i, point) in path.iter().enumerate() {
        let offset = i * shape_vertex_count;
        for j in 0..shape_vertex_count {
            let id = offset + j;
            mesh_vertices[id] = point.local_to_world(Vec3::from_array(shape.vertices[j])).to_array();
            mesh_normals[id] = point.local_to_world_direction(Vec3::from_array(shape.normals[j])).to_array();
            if !shape.u_coords.is_empty() {
                mesh_uvs[id] = [shape.u_coords[j], point.v_coordinate];
            }
        }
    }

    // Extruded indices
    let mut tri_index = 0;
    for i in 0..segments {
        let offset = i * shape_vertex_count;
        for j in (0..shape.edges.len()).step_by(2) {
            let a = offset + shape.edges[j] as usize + shape_vertex_count;
            let b = offset + shape.edges[j] as usize;
            let c = offset + shape.edges[j+1] as usize;
            let d = offset + shape.edges[j+1] as usize + shape_vertex_count;

            mesh_indices[tri_index] = a as u32; tri_index += 1;
            mesh_indices[tri_index] = b as u32; tri_index += 1;
            mesh_indices[tri_index] = c as u32; tri_index += 1;
            mesh_indices[tri_index] = c as u32; tri_index += 1;
            mesh_indices[tri_index] = d as u32; tri_index += 1;
            mesh_indices[tri_index] = a as u32; tri_index += 1;
        }
    }

    mesh_indices.reverse();

    // Construct the mesh
    let mut mesh = Mesh::new(PrimitiveTopology::TriangleList, RenderAssetUsages::default());
    mesh.insert_indices(Indices::U32(mesh_indices));
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, mesh_vertices);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, mesh_normals);
    if !shape.u_coords.is_empty() {
        mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, mesh_uvs);
    }

    mesh
}
