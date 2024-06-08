


trait System{
    /// Data = (Com1,Com2,...) 
    type Data;

    //depend : {System1,System2} // System1,System2 先执行后执行当前System,(当前System依赖System1和System2)


    fn update();
}
