use serde::Deserialize;

use siyuan_types::SiyuanError;

#[derive(Debug, Deserialize)]
pub struct SiyuanResponse<T> {
    pub code: i32,
    #[serde(default)]
    pub msg: String,
    #[serde(default = "Option::default")]
    pub data: Option<T>,
}

impl<T> SiyuanResponse<T> {
    pub fn into_result(self) -> Result<T, SiyuanError> {
        if self.code == 0 {
            self.data.ok_or_else(|| {
                SiyuanError::Parse("kernel returned code=0 but no data field".into())
            })
        } else {
            Err(SiyuanError::Api {
                code: self.code,
                msg: self.msg,
            })
        }
    }

    /// Some endpoints (e.g. removeNotebook) legitimately return `data: null`
    /// on success.
    pub fn into_result_or_unit(self) -> Result<Option<T>, SiyuanError> {
        if self.code == 0 {
            Ok(self.data)
        } else {
            Err(SiyuanError::Api {
                code: self.code,
                msg: self.msg,
            })
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn happy_path_returns_data() {
        let raw = r#"{"code":0,"msg":"","data":{"v":42}}"#;
        let r: SiyuanResponse<serde_json::Value> = serde_json::from_str(raw).unwrap();
        let out = r.into_result().unwrap();
        assert_eq!(out["v"], 42);
    }

    #[test]
    fn nonzero_code_becomes_api_error() {
        let raw = r#"{"code":21,"msg":"bad","data":null}"#;
        let r: SiyuanResponse<serde_json::Value> = serde_json::from_str(raw).unwrap();
        match r.into_result() {
            Err(SiyuanError::Api { code, msg }) => {
                assert_eq!(code, 21);
                assert_eq!(msg, "bad");
            }
            other => panic!("unexpected: {other:?}"),
        }
    }

    #[test]
    fn null_data_with_code_zero_is_unit_ok() {
        let raw = r#"{"code":0,"msg":"","data":null}"#;
        let r: SiyuanResponse<serde_json::Value> = serde_json::from_str(raw).unwrap();
        assert!(r.into_result_or_unit().unwrap().is_none());
    }
}
