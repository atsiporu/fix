pub mod header
{
    pub const Version: u32 = 8;
    pub const Length: u32 = 9;
    pub const MsgType: u32 = 35;
}

pub mod body 
{
    pub const Text: u32 = 58;
}

pub mod trailer
{
    pub const CheckSum: u32 = 10;
}
