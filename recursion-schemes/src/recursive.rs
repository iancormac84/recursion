use std::sync::Arc;

#[cfg(feature = "backcompat")]
use recursion::Collapse;

use crate::functor::{Compose, Functor, FunctorExt, PartiallyApplied};

pub trait Recursive
where
    Self: Sized,
{
    type FunctorToken: Functor;

    fn into_layer(self) -> <Self::FunctorToken as Functor>::Layer<Self>;
}

/// heap allocated fix point of some Functor
pub struct Fix<F: Functor>(pub Box<F::Layer<Fix<F>>>);

// recursing over a fix point structure is free
impl<F: Functor> Recursive for Fix<F> {
    type FunctorToken = F;

    fn into_layer(self) -> <Self::FunctorToken as Functor>::Layer<Self> {
        *self.0
    }
}

// TODO: futumorphism to allow for partial non-async expansion? yes! but (I think) needs to be erased for collapse phase
// TODO: b/c at that point there's no need for that info..

pub struct WithContext<R: Recursive>(pub R);

impl<R: Recursive + Copy> Recursive for WithContext<R> {
    type FunctorToken = Compose<R::FunctorToken, (R, PartiallyApplied)>;

    fn into_layer(self) -> <Self::FunctorToken as Functor>::Layer<Self> {
        let layer = R::into_layer(self.0);
        R::FunctorToken::fmap(layer, move |wrapped| (wrapped, WithContext(wrapped)))
    }
}

pub struct PartialExpansion<R: Recursive> {
    pub wrapped: R,
    #[allow(clippy::type_complexity)]
    pub f: Arc<
        // TODO: probably doesn't need to be an arc but (shrug emoji)
        dyn Fn(
            <<R as Recursive>::FunctorToken as Functor>::Layer<R>,
        ) -> <<R as Recursive>::FunctorToken as Functor>::Layer<Option<R>>,
    >,
}

impl<R: Recursive> Recursive for PartialExpansion<R> {
    type FunctorToken = Compose<R::FunctorToken, Option<PartiallyApplied>>;

    fn into_layer(self) -> <Self::FunctorToken as Functor>::Layer<Self> {
        let partially_expanded = (self.f)(self.wrapped.into_layer());
        Self::FunctorToken::fmap(partially_expanded, move |wrapped| PartialExpansion {
            wrapped,
            f: self.f.clone(),
        })
    }
}

pub trait RecursiveExt: Recursive {
    fn fold_recursive<
        Out,
        F: FnMut(<<Self as Recursive>::FunctorToken as Functor>::Layer<Out>) -> Out,
    >(
        self,
        collapse_layer: F,
    ) -> Out;

    fn expand_and_collapse<Seed, Out>(
        seed: Seed,
        expand_layer: impl FnMut(Seed) -> <<Self as Recursive>::FunctorToken as Functor>::Layer<Seed>,
        collapse_layer: impl FnMut(<<Self as Recursive>::FunctorToken as Functor>::Layer<Out>) -> Out,
    ) -> Out;
}

impl<X> RecursiveExt for X
where
    X: Recursive,
{
    fn fold_recursive<
        Out,
        F: FnMut(<<X as Recursive>::FunctorToken as Functor>::Layer<Out>) -> Out,
    >(
        self,
        collapse_layer: F,
    ) -> Out {
        Self::expand_and_collapse(self, Self::into_layer, collapse_layer)
    }

    fn expand_and_collapse<Seed, Out>(
        seed: Seed,
        expand_layer: impl FnMut(Seed) -> <<X as Recursive>::FunctorToken as Functor>::Layer<Seed>,
        collapse_layer: impl FnMut(<<X as Recursive>::FunctorToken as Functor>::Layer<Out>) -> Out,
    ) -> Out {
        <X as Recursive>::FunctorToken::expand_and_collapse(seed, expand_layer, collapse_layer)
    }
}

#[cfg(feature = "backcompat")]
struct CollapseViaRecursive<X>(X);

#[cfg(feature = "backcompat")]
impl<Out, R: RecursiveExt> Collapse<Out, <<R as Recursive>::FunctorToken as Functor>::Layer<Out>>
    for CollapseViaRecursive<R>
{
    fn collapse_layers<F: FnMut(<<R as Recursive>::FunctorToken as Functor>::Layer<Out>) -> Out>(
        self,
        collapse_layer: F,
    ) -> Out {
        self.0.fold_recursive(collapse_layer)
    }
}
