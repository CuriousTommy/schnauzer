use crate::ObjectType;
use crate::Parser;
use crate::Result;
use crate::reader::ReaderBuildOption;

pub(crate) fn load_object_type_with(option: ReaderBuildOption) -> Result<ObjectType> {
    let parser = Parser::build(option)?;
    let object = parser.parse()?;

    Ok(object)
}
