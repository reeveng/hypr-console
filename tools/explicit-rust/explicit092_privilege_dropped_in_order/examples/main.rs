fn initgroups(_named: &str, _group: u32) -> Result<(), u8> {
    Ok(())
}

fn setgid(_group: u32) -> Result<(), u8> {
    Ok(())
}

fn setuid(_user: u32) -> Result<(), u8> {
    Ok(())
}

fn user_first(user: u32, group: u32) -> Result<(), u8> {
    //~v ERROR EXPLICIT092_PRIVILEGE_DROPPED_IN_ORDER
    setuid(user)?;
    //~v ERROR EXPLICIT092_PRIVILEGE_DROPPED_IN_ORDER
    setgid(group)?;

    Ok(())
}

fn groups_kept(user: u32, group: u32) -> Result<(), u8> {
    //~v ERROR EXPLICIT092_PRIVILEGE_DROPPED_IN_ORDER
    setgid(group)?;
    //~v ERROR EXPLICIT092_PRIVILEGE_DROPPED_IN_ORDER
    setuid(user)?;

    Ok(())
}

fn group_kept(named: &str, user: u32, group: u32) -> Result<(), u8> {
    initgroups(named, group)?;
    //~v ERROR EXPLICIT092_PRIVILEGE_DROPPED_IN_ORDER
    setuid(user)?;

    Ok(())
}

fn in_order(named: &str, user: u32, group: u32) -> Result<(), u8> {
    initgroups(named, group)?;
    setgid(group)?;
    setuid(user)?;

    Ok(())
}

fn main() {
    let _ = user_first(1000, 1000);
    let _ = groups_kept(1000, 1000);
    let _ = group_kept("somebody", 1000, 1000);
    let _ = in_order("somebody", 1000, 1000);
}
