//! # Authors
//! greenhand520
//! # Since
//! version: 0.1.0
//! # Date
//! 2026/7/3 11:26

/// CRC-16/CCITT (与 STM32 端完全一致)
pub fn crc16_ccitt(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        crc ^= (byte as u16) << 8;
        for _ in 0..8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
        }
    }
    crc
}

/// 预计算查找表（编译期完成，零运行时开销）
const CRC16_TABLE: [u16; 256] = {
    let mut table = [0u16; 256];
    let mut i = 0usize;
    while i < 256 {
        // 把 i 当作一个字节，模拟逐位法算出它的 CRC
        let mut crc = (i as u16) << 8;
        let mut j = 0;
        while j < 8 {
            crc = if crc & 0x8000 != 0 {
                (crc << 1) ^ 0x1021
            } else {
                crc << 1
            };
            j += 1;
        }
        table[i] = crc;
        i += 1;
    }
    table
};

/// 查表法计算 CRC-16/CCITT
pub fn crc16_ccitt_table(data: &[u8]) -> u16 {
    let mut crc: u16 = 0xFFFF;
    for &byte in data {
        // 用 crc 高字节 ^ 输入字节 作为查表索引
        let index = ((crc >> 8) ^ (byte as u16)) as u8;
        // crc 左移 8 位，再异或查表结果
        crc = (crc << 8) ^ CRC16_TABLE[index as usize];
    }
    crc
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_crc_known_values() {
        // 空数据
        assert_eq!(crc16_ccitt(&[]), 0xFFFF);
        // "123456789" 标准测试向量
        assert_eq!(crc16_ccitt(b"123456789"), 0x29B1);
        assert_eq!(crc16_ccitt_table(b"123456789"), 0x29B1);
    }
}
