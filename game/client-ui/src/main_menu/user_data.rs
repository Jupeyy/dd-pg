use client_types::server_browser::ServerBrowserData;

pub struct UserData<'a> {
    pub browser_data: &'a mut ServerBrowserData,
}
