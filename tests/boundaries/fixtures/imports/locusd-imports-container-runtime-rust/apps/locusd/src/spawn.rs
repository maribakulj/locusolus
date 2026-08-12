use bollard::Docker;

pub fn daemon() -> Docker {
    Docker::connect_with_socket_defaults().expect("socket")
}
