#![allow(missing_docs)]

use http::Method;
use openapiv3::{OpenAPI, PathItem, ReferenceOr};

use crate::filter::{Filter, FilterBase, Internal};

use std::{any::TypeId, collections::HashMap, fmt::Debug, iter::IntoIterator};

#[derive(Clone, Debug, Default)]
pub struct RouteDocumentation {
    pub cookies: Vec<DocumentedCookie>,
    pub headers: Vec<DocumentedHeader>,
    pub method: Option<Method>,
    pub parameters: Vec<DocumentedParameter>,
    pub path: String,
    pub queries: Vec<DocumentedQuery>,
    pub responses: HashMap<u16, DocumentedResponse>,
}

#[derive(Clone, Debug)]
pub struct DocumentedCookie {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Clone, Debug)]
pub struct DocumentedHeader {
    pub name: String,
    pub description: Option<String>,
    pub required: bool,
}

#[derive(Clone, Debug)]
pub struct DocumentedParameter {
    pub name: String,
    pub description: Option<String>,
    pub parameter_type: DocumentedType,
}

#[derive(Clone, Debug)]
pub struct DocumentedQuery {
    pub name: String,
    pub description: Option<String>,
    pub parameter_type: DocumentedType,
    pub required: bool,
}

#[derive(Clone, Debug, Default)]
pub struct DocumentedResponse {
    pub description: String,
    pub headers: Vec<DocumentedHeader>,
    pub body: Vec<DocumentedResponseBody>,
}

#[derive(Clone, Debug)]
pub struct DocumentedResponseBody {
    pub body: DocumentedType,
    pub mime: Option<String>,
}
impl Default for DocumentedResponseBody {
    fn default() -> Self {
        Self {
            body: DocumentedType::Object(HashMap::default()),
            mime: None,
        }
    }
}

#[derive(Clone, Debug)]
pub enum DocumentedType {
    Array(Box<DocumentedType>),
    Object(HashMap<String, DocumentedType>),
    Primitive{ ty: InternalDocumentedType, documentation: Option<String>, required: bool},
}
impl DocumentedType {
    pub fn boolean() -> Self {
        Self::Primitive{ ty: InternalDocumentedType::Boolean, documentation: None, required: true }
    }
    pub fn float() -> Self {
        Self::Primitive{ ty: InternalDocumentedType::Float, documentation: None, required: true }
    }
    pub fn integer() -> Self {
        Self::Primitive{ ty: InternalDocumentedType::Integer, documentation: None, required: true }
    }
    pub fn string() -> Self {
        Self::Primitive{ ty: InternalDocumentedType::String, documentation: None, required: true }
    }
    pub fn object(fields: HashMap<String, DocumentedType>) -> Self {
        Self::Object(fields)
    }
}

#[derive(Clone, Debug)]
pub enum InternalDocumentedType {
    Boolean,
    Float,
    Integer,
    String,
}
impl From<TypeId> for DocumentedType {
    fn from(id: TypeId) -> Self {
        // A HashMap initialised with Once might be better.
        match id {
            t if t == TypeId::of::<u8>() => Self::integer(),
            t if t == TypeId::of::<u16>() => Self::integer(),
            t if t == TypeId::of::<u32>() => Self::integer(),
            t if t == TypeId::of::<u64>() => Self::integer(),
            t if t == TypeId::of::<u128>() => Self::integer(),
            t if t == TypeId::of::<i8>() => Self::integer(),
            t if t == TypeId::of::<i16>() => Self::integer(),
            t if t == TypeId::of::<i32>() => Self::integer(),
            t if t == TypeId::of::<i64>() => Self::integer(),
            t if t == TypeId::of::<i128>() => Self::integer(),
            t if t == TypeId::of::<String>() => Self::string(),
            _ => Self::object(HashMap::default()),
        }
    }
}

pub fn describe<F: Filter>(filter: F) -> Vec<RouteDocumentation> {
    let mut routes = filter.describe(RouteDocumentation::default());
    routes.iter_mut()
        .filter(|route| route.path.is_empty())
        .for_each(|route| route.path.push('/'));
    routes
}

#[derive(Copy, Clone, Debug)]
pub struct ExplicitDocumentation<T, F>
where F: Fn(&mut RouteDocumentation) {
    item: T,
    callback: F,
}
impl<T, F: Fn(&mut RouteDocumentation)> ExplicitDocumentation<T, F> {
    pub fn new(item: T, callback: F) -> Self {
        ExplicitDocumentation{ item, callback }
    }
}
impl<T, F: Fn(&mut RouteDocumentation)> FilterBase for ExplicitDocumentation<T, F>
where T: FilterBase {
    type Extract = T::Extract;
    type Error = T::Error;
    type Future = T::Future;
    
    fn filter(&self, internal: Internal) -> Self::Future {
        self.item.filter(internal)
    }

    fn describe(&self, mut route: RouteDocumentation) -> Vec<RouteDocumentation> {
        let ExplicitDocumentation{ callback, .. } = self;
        (callback)(&mut route);
        vec![route]
    }
}

pub fn to_openapi(routes: Vec<RouteDocumentation>) -> OpenAPI {
    use openapiv3::{ArrayType, Header, IntegerType, MediaType, NumberType, ObjectType, Operation, Parameter, ParameterData, ParameterSchemaOrContent, PathStyle, Response, Schema, SchemaData, SchemaKind, StatusCode, StringType, Type as OpenApiType};

    let paths = routes.into_iter()
        .map(|route| {
            let RouteDocumentation{
                cookies,
                headers,
                method,
                parameters,
                mut path,
                queries,
                responses
            } = route;
            let mut item = PathItem::default();
            let mut operation = Operation::default();

            fn documented_type_to_openapi(t: DocumentedType) -> Schema {
                match t {
                    DocumentedType::Array(i) => {
                        Schema {
                            schema_data: SchemaData::default(),
                            schema_kind: SchemaKind::Type(OpenApiType::Array(ArrayType{
                                items: ReferenceOr::Item(Box::new(documented_type_to_openapi(*i))),
                                min_items: None,
                                max_items: None,
                                unique_items: false,
                            }))
                        }
                    }
                    DocumentedType::Object(p) => {
                        Schema {
                            schema_data: SchemaData::default(),
                            schema_kind: SchemaKind::Type(OpenApiType::Object(ObjectType{
                                properties: p.into_iter()
                                    .map(|(name, type_)| (name, ReferenceOr::Item(Box::new(documented_type_to_openapi(type_)))))
                                    .collect(),
                                ..ObjectType::default()
                            }))
                        }
                    }
                    DocumentedType::Primitive{ty, documentation, required} => {
                        Schema {
                            schema_data: SchemaData{
                                description: documentation,
                                nullable: !required,
                                ..SchemaData::default()
                            },
                            schema_kind: SchemaKind::Type(match ty {
                                InternalDocumentedType::Boolean => OpenApiType::Boolean{},
                                InternalDocumentedType::Float => OpenApiType::Number(NumberType::default()),
                                InternalDocumentedType::Integer => OpenApiType::Integer(IntegerType::default()),
                                InternalDocumentedType::String => OpenApiType::String(StringType::default()),
                            }),
                        }
                    }
                }
            }

            operation.parameters.extend(
                parameters.into_iter()
                    .enumerate()
                    .inspect(|(i, param)| path = path.replace(format!("{{{}}}", i).as_str(), format!("{{{}}}", param.name).as_str()))
                    .map(|(_, param)| ReferenceOr::Item(Parameter::Path{style: PathStyle::default(), parameter_data: ParameterData{
                        name: param.name,
                        description: param.description,
                        required: true,
                        deprecated: Some(false),
                        format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(documented_type_to_openapi(param.parameter_type))),
                        example: None,
                        examples: Default::default(),
                    }}))
            );
            operation.parameters.extend(
                headers.into_iter()
                    .map(|header| ReferenceOr::Item(Parameter::Header{style: Default::default(), parameter_data: ParameterData{
                        name: header.name,
                        description: header.description,
                        required: header.required,
                        deprecated: Some(false),
                        format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(Schema{
                            schema_data: SchemaData::default(),
                            schema_kind: SchemaKind::Type(OpenApiType::String(StringType::default())),
                        })),
                        example: None,
                        examples: Default::default(),
                    }}))
            );
            operation.parameters.extend(
                queries.into_iter()
                    .map(|query| ReferenceOr::Item(Parameter::Query{
                        style: Default::default(),
                        allow_reserved: false,
                        allow_empty_value: None,
                        parameter_data: ParameterData{
                            name: query.name,
                            description: query.description,
                            required: query.required,
                            deprecated: Some(false),
                            format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(Schema{
                                schema_data: SchemaData::default(),
                                schema_kind: SchemaKind::Type(OpenApiType::String(StringType::default())),
                            })),
                            example: None,
                            examples: Default::default(),
                        },
                    }))
            );
            operation.parameters.extend(
                cookies.into_iter()
                    .map(|cookie| ReferenceOr::Item(Parameter::Cookie{
                        style: Default::default(),
                        parameter_data: ParameterData {
                            name: cookie.name,
                            description: cookie.description,
                            required: cookie.required,
                            deprecated: Some(false),
                            format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(Schema{
                                schema_data: SchemaData::default(),
                                schema_kind: SchemaKind::Type(OpenApiType::String(StringType::default())),
                            })),
                            example: None,
                            examples: Default::default(),
                        }
                    }))
            );

            let mut responses = responses.into_iter().collect::<Vec<_>>();
            responses.sort_by_key(|(code, _)| *code);
            operation.responses.responses.extend(
                responses.into_iter()
                    .map(|(code, response)| (StatusCode::Code(code), ReferenceOr::Item(Response{
                        description: response.description,
                        headers: response.headers.into_iter().map(|header| (header.name, ReferenceOr::Item(Header{
                            description: header.description,
                            style: Default::default(),
                            required: false,
                            deprecated: None,
                            format: ParameterSchemaOrContent::Schema(ReferenceOr::Item(Schema{
                                schema_kind: SchemaKind::Type(OpenApiType::String(Default::default())),
                                schema_data: SchemaData::default(),
                            })),
                            example: None,
                            examples: Default::default(),
                        }))).collect(),
                        content: response.body.into_iter().map(|body| (body.mime.unwrap_or("*/*".into()), MediaType{
                            example: None,
                            examples: Default::default(),
                            encoding: Default::default(),
                            schema: Some(ReferenceOr::Item(documented_type_to_openapi(body.body)))
                        })).collect(),
                        ..Response::default()
                    })))
            );

            match method.unwrap_or(Method::POST) {
                Method::GET => item.get = Some(operation),
                Method::POST => item.post = Some(operation),
                Method::PUT => item.put = Some(operation),
                Method::DELETE => item.delete = Some(operation),
                Method::HEAD => item.head = Some(operation),
                Method::OPTIONS => item.options = Some(operation),
                Method::PATCH => item.patch = Some(operation),
                Method::TRACE => item.trace = Some(operation),
                _ => unimplemented!(),
            }

            (path, ReferenceOr::Item(item))
        }).collect();
    
    OpenAPI {
        openapi: "3.0.0".into(),
        paths,
        ..OpenAPI::default()
    }
}
