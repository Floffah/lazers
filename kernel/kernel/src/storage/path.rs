use super::StorageError;

const MAX_PATH_COMPONENTS: usize = 64;

/// Canonicalizes one runtime path against a process-owned cwd.
pub fn normalize_path(cwd: &str, path: &str, buffer: &mut [u8]) -> Result<usize, StorageError> {
    if path.is_empty() {
        return Err(StorageError::InvalidPath);
    }

    let mut normalized_len = 1usize;
    let mut component_count = 0usize;
    let mut component_starts = [0usize; MAX_PATH_COMPONENTS];
    buffer[0] = b'/';

    if path.starts_with('/') {
        normalize_path_parts(
            path,
            buffer,
            &mut normalized_len,
            &mut component_starts,
            &mut component_count,
        )?;
    } else {
        normalize_path_parts(
            cwd,
            buffer,
            &mut normalized_len,
            &mut component_starts,
            &mut component_count,
        )?;
        normalize_path_parts(
            path,
            buffer,
            &mut normalized_len,
            &mut component_starts,
            &mut component_count,
        )?;
    }

    Ok(normalized_len)
}

fn normalize_path_parts(
    source: &str,
    buffer: &mut [u8],
    normalized_len: &mut usize,
    component_starts: &mut [usize; MAX_PATH_COMPONENTS],
    component_count: &mut usize,
) -> Result<(), StorageError> {
    for component in source.split('/') {
        if component.is_empty() || component == "." {
            continue;
        }

        if component == ".." {
            if *component_count > 0 {
                *component_count -= 1;
                *normalized_len = component_starts[*component_count];
            }
            continue;
        }

        if *component_count >= component_starts.len() {
            return Err(StorageError::InvalidPath);
        }

        let previous_len = *normalized_len;
        let required = if previous_len == 1 {
            component.len()
        } else {
            component.len() + 1
        };
        if previous_len + required > buffer.len() {
            return Err(StorageError::InvalidPath);
        }

        if previous_len > 1 {
            buffer[*normalized_len] = b'/';
            *normalized_len += 1;
        }

        buffer[*normalized_len..*normalized_len + component.len()]
            .copy_from_slice(component.as_bytes());
        *normalized_len += component.len();
        component_starts[*component_count] = previous_len;
        *component_count += 1;
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::normalize_path;
    use crate::storage::StorageError;
    use std::string::{String, ToString};
    use std::vec;
    use std::vec::Vec;

    fn normalized(cwd: &str, path: &str, buffer_len: usize) -> Result<String, StorageError> {
        let mut buffer = vec![0u8; buffer_len];
        let len = normalize_path(cwd, path, &mut buffer)?;
        Ok(core::str::from_utf8(&buffer[..len]).unwrap().to_string())
    }

    #[test]
    fn preserves_absolute_paths() {
        assert_eq!(normalized("/system", "/bin/echo", 64).unwrap(), "/bin/echo");
    }

    #[test]
    fn resolves_relative_paths_against_cwd() {
        assert_eq!(
            normalized("/system/bin", "echo", 64).unwrap(),
            "/system/bin/echo"
        );
    }

    #[test]
    fn removes_current_directory_segments() {
        assert_eq!(
            normalized("/system/bin", "./echo", 64).unwrap(),
            "/system/bin/echo"
        );
    }

    #[test]
    fn resolves_parent_segments() {
        assert_eq!(
            normalized("/system/bin", "../cat", 64).unwrap(),
            "/system/cat"
        );
    }

    #[test]
    fn collapses_repeated_slashes() {
        assert_eq!(
            normalized("/system//bin", "///echo///", 64).unwrap(),
            "/echo"
        );
    }

    #[test]
    fn clamps_parent_walks_at_root() {
        assert_eq!(normalized("/", "../../../../echo", 64).unwrap(), "/echo");
    }

    #[test]
    fn rejects_empty_input() {
        let mut buffer = [0u8; 16];
        assert!(matches!(
            normalize_path("/system", "", &mut buffer),
            Err(StorageError::InvalidPath)
        ));
    }

    #[test]
    fn rejects_too_many_components() {
        let path = core::iter::repeat_n("a", 65).collect::<Vec<_>>().join("/");
        let mut buffer = [0u8; 256];
        assert!(matches!(
            normalize_path("/", &path, &mut buffer),
            Err(StorageError::InvalidPath)
        ));
    }

    #[test]
    fn rejects_too_small_output_buffer() {
        let mut buffer = [0u8; 4];
        assert!(matches!(
            normalize_path("/system/bin", "echo", &mut buffer),
            Err(StorageError::InvalidPath)
        ));
    }
}
