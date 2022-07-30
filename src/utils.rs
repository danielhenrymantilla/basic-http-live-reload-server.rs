#[extension(pub(in crate) trait Also)]
impl<T> T {
    fn also (mut self, also: impl FnOnce(&mut Self))
      -> Self
    {
        also(&mut self);
        self
    }
}
