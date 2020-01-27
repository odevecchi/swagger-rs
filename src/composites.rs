//! Module for combining hyper services
//!
//! Use by passing `hyper::server::MakeService` instances to a `CompositeMakeService`
//! together with the base path for requests that should be handled by that service.
use hyper::service::Service;
use hyper::{Request, Response, StatusCode};
use std::ops::{Deref, DerefMut};
use std::{fmt, io};

/// Trait for generating a default "not found" response. Must be implemented on
/// the `Response` associated type for `MakeService`s being combined in a
/// `CompositeMakeService`.
pub trait NotFound<V> {
    /// Return a "not found" response
    fn not_found() -> hyper::Response<V>;
}

impl <B: Default> NotFound<Body> for Body {
    fn not_found() -> hyper::Response<Body> {
        Response::builder()
            .status(StatusCode::NOT_FOUND)
            .body(Body::default())
            .unwrap()
    }
}

/// Wraps a vector of pairs, each consisting of a base path as a `&'static str`
/// and a `MakeService` instance. Implements `Deref<Vec>` and `DerefMut<Vec>` so
/// these can be manipulated using standard `Vec` methods.
///
/// The `Service` returned by calling `make_service()` will pass an incoming
/// request to the first `Service` in the list for which the associated
/// base path is a prefix of the request path.
///
/// Example Usage
/// =============
///
/// ```ignore
/// let my_make_service1 = MakeService1::new();
/// let my_make_service2 = MakeService2::new();
///
/// let mut composite_make_service = CompositeMakeService::new();
/// composite_make_service.push(("/base/path/1", my_make_service1));
/// composite_make_service.push(("/base/path/2", my_make_service2));
///
/// // use as you would any `MakeService` instance
/// ```
type CompositedService<ReqBody, ResBody, Error> = Box<dyn Service<Request<ReqBody>, Response=Response<ResBody>, Error=Error>>;
type CompositeMakeSeviceVec<T, SE, ReqBody, ResBody, RE> = Vec<(&'static str, Box<dyn Service<T, Error=SE, Response=CompositedService<ReqBody, ResBody, RE>>>)>;

#[derive(Default)]
pub struct CompositeMakeService<Target, ServiceError, ReqBody, ResBody, ReqError>
{
    inner: CompositeMakeServiceVec,
    phantom: PhantomData<(ServiceError, ReqBody, ResBody, ReqError)>
}

impl<Target, ServiceError, ReqBody, ResBody, ReqError> CompositeMakeService<Target, ServiceError, ReqBody, ResBody, ReqError> {
    /// create an empty `CompositeMakeService`
    pub fn new() -> Self {
        CompositeMakeService {
          inner: Vec::new()
        }
    }
}

impl<Target, ServiceError, ReqBody, ResBody, ReqError> Service<Target> for CompositeMakeService<Target, ServiceError, ReqBody, ResBody, ReqError>
{
    type Error = ServiceError;
    type Response = CompositeService<ReqBody, ResBody, ReqError>;
    type Future = futures::future::FutureResult<Self::Service, io::Error>;

    fn call(
        &mut self,
        target: Target,
    ) -> futures::future::FutureResult<Self::Service, io::Error> {

        futures::future::join_all(self.inner.iter().map(|(path, service)| service.call(context).map(|i| (path, i)))).map(|services| Ok(CompositeService(services)))
    }
}

impl<Target, ServiceError, ReqBody, ResBody, ReqError> fmt::Debug for CompositeMakeService<Target, ServiceError, ReqBody, ResBody, ReqError>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        // Get vector of base paths
        let str_vec: Vec<&'static str> = self.0.iter().map(|&(base_path, _)| base_path).collect();
        write!(
            f,
            "CompositeMakeService accepting base paths: {:?}",
            str_vec,
        )
    }
}

impl<Target, ServiceError, ReqBody, ResBody, ReqError> Deref for CompositeMakeService<Target, ServiceError, ReqBody, ResBody, ReqError>
{
    type Target = CompositeMakeServiceVec<Target>;

    fn deref(&self) -> &Self::Target {
        &self.inner
    }
}

impl<Target, ServiceError, ReqBody, ResBody, ReqError> DerefMut for CompositeMakeService<Target, ServiceError, ReqBody, ResBody, ReqError>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}

/// Wraps a vector of pairs, each consisting of a base path as a `&'static str`
/// and a `Service` instance.
pub struct CompositeService<ReqBody, ResBody, Error>(Vec<(&'static str, BoxedService<ReqBody, ResBody, Error>)>)
where
    V: NotFound<V> + 'static,
    W: 'static;

impl<ReqBody, ResBody, Error> Service<Request<ReqBody>> for CompositeService<ReqBody, ResBody, Error>
{
    type Error = Error;
    type Response = Response<ResBody>;
    type Future = Box<dyn Future<Item = Response<ResBody>, Error = Error>>;

    fn call(&mut self, req: Request<Self::ReqBody>) -> Self::Future {
        for &mut (base_path, ref mut service) in &mut self.0 {
            if req.uri().path().starts_with(base_path) {
                return service.call(req);
            }
        }

        Box::new(future::ok(V::not_found()))
    }
}

impl<ReqBody, ResBody, Error> fmt::Debug for CompositeService<ReqBody, ResBody, Error>
{
    fn fmt(&self, f: &mut fmt::Formatter) -> Result<(), fmt::Error> {
        // Get vector of base paths
        let str_vec: Vec<&'static str> = self.0.iter().map(|&(base_path, _)| base_path).collect();
        write!(f, "CompositeService accepting base paths: {:?}", str_vec,)
    }
}

impl<ReqBody, ResBody, Error> Deref for CompositeService<ReqBody, ResBody, Error>
{
    type Target = Vec<(&'static str, BoxedService<ReqBody, ResBody, Error>)>;
    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl<ReqBody, ResBody, Error> DerefMut for CompositeService<ReqBody, ResBody, Error>
{
    fn deref_mut(&mut self) -> &mut Self::Target {
        &mut self.0
    }
}
