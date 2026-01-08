#[path = "../../benches/bip.rs"]
#[macro_use]
mod bip;

create_bip_benchmark! {
    "rtrb",
    rtrb::bip_arc::RingBuffer::new,
    |p, s| p.write_chunk(s.len()).map(|mut chunk| {
        chunk.as_mut_slice().copy_from_slice(s);
        chunk.commit_all();
    }).is_ok(),
    |c, s| c.read_chunk(s.len()).map(|chunk| {
        s.copy_from_slice(chunk.as_slice());
        chunk.commit_all();
    }).is_ok(),
    ::
    "bbqueue",
    |capacity| {
        use bbqueue::traits::storage::BoxedSlice;
        use bbqueue::nicknames::GogiGui;
        let rb = GogiGui::new_with_storage(BoxedSlice::new(capacity));
        let p = rb.stream_producer();
        let c = rb.stream_consumer();
        (p, c)
    },
    |p, s| p.grant_exact(s.len()).map(|mut wgr| {
        wgr.copy_from_slice(s);
        wgr.commit(s.len());
    }).is_ok(),
    |c, s| {
        if let Ok(rgr) = c.read() {
            if rgr.len() < s.len() {
                false
            } else {
                s.copy_from_slice(&rgr[..s.len()]);
                rgr.release(s.len());
                true
            }
        } else {
            false
        }
    },
    ::
}
