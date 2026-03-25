pub fn add(left: u64, right: u64) -> u64 {
    left + right
}

#[cfg(feature = "python")]
#[pyo3::pymodule(name = "tonner")]
mod tonner_py {
    
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn it_works() {
        let result = add(2, 2);
        assert_eq!(result, 4);
    }
}
