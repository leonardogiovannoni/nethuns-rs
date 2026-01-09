// Frontend Distributor logic for nethuns-rs
// This module contains the frontend API traits and implementations for distributing packets
// across multiple consumers in a single-producer, multiple-consumer (SPMC) pattern.

use std::cell::RefCell;

use super::{Context, Payload, Token};
use crate::api::bdistributor::{
    BDistributor, PopError, PushError, TryPopError, TryPushError,
    nspscbdistributor::{NSPSCBDistributor, NSPSCBDistributorPopper, NSPSCBDistributorPusher},
    spmcbdistributor::{SPMCBDistributor, SPMCBDistributorPopper, SPMCBDistributorPusher},
};

/// A wrapper that combines context with distributor data
pub struct ContextWrapper<const BATCH_SIZE: usize, T, Ctx: Context> {
    pub ctx: Ctx,
    pub data: RefCell<T>,
}

/// Base trait for distributors
pub trait Distributor<const N: usize, Ctx: Context>: Send + Sized {
    type Inner: BDistributor<N, Token>;

    fn create(inner: Self::Inner, ctx: Ctx) -> Self;
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> Distributor<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: BDistributor<BATCH_SIZE, Token>,
{
    type Inner = T;

    fn create(inner: Self::Inner, ctx: Ctx) -> Self {
        Self {
            ctx,
            data: RefCell::new(inner),
        }
    }
}


// SPMC distributor traits
pub trait SPMCDistributorPusher<const BATCH_SIZE: usize, Ctx: Context> {
    fn try_push(
        &self,
        batch: [Payload<'_, Ctx>; BATCH_SIZE],
    ) -> core::result::Result<(), TryPushError<[Payload<'_, Ctx>; BATCH_SIZE]>>;

    fn push(
        &self,
        batch: [Payload<'_, Ctx>; BATCH_SIZE],
    ) -> impl core::future::Future<
        Output = core::result::Result<(), PushError<[Payload<'_, Ctx>; BATCH_SIZE]>>,
    >;
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> SPMCDistributorPusher<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: SPMCBDistributorPusher<BATCH_SIZE, Token>,
{
    fn try_push(
        &self,
        batch: [Payload<'_, Ctx>; BATCH_SIZE],
    ) -> core::result::Result<(), TryPushError<[Payload<'_, Ctx>; BATCH_SIZE]>> {
        let erased = batch.map(|x| {
            let rv = x.into_token();
            if super::unlikely(!rv.check_token(&self.ctx)) {
                panic!("Wrong context");
            }
            rv
        });

        self.data
            .borrow_mut()
            .try_push(erased)
            .map_err(|err| match err {
                TryPushError::Full(batch) => {
                    TryPushError::Full(batch.map(|x| x.consume(&self.ctx)))
                }
                TryPushError::Closed(batch) => {
                    TryPushError::Closed(batch.map(|x| x.consume(&self.ctx)))
                }
            })
    }

    fn push(
        &self,
        batch: [Payload<'_, Ctx>; BATCH_SIZE],
    ) -> impl core::future::Future<
        Output = core::result::Result<(), PushError<[Payload<'_, Ctx>; BATCH_SIZE]>>,
    > {
        let ctx = &self.ctx;
        let erased = batch.map(|x| {
            let rv = x.into_token();
            if super::unlikely(!rv.check_token(ctx)) {
                panic!("Wrong context");
            }
            rv
        });
        async move {
            let mut data = self.data.borrow_mut();
            match data.push(erased).await {
                Ok(()) => Ok(()),
                Err(err) => Err(PushError(err.into_inner().map(|x| x.consume(ctx)))),
            }
        }
    }
}

/// Trait for popping packets from an SPMC distributor (frontend API)
pub trait SPMCDistributorPopper<const BATCH_SIZE: usize, Ctx: Context> {
    fn try_pop(&self) -> Result<[Payload<'_, Ctx>; BATCH_SIZE], TryPopError>;

    fn pop(
        &self,
    ) -> impl core::future::Future<Output = Result<[Payload<'_, Ctx>; BATCH_SIZE], PopError>>;
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> SPMCDistributorPopper<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: SPMCBDistributorPopper<BATCH_SIZE, Token>,
{
    fn try_pop(&self) -> Result<[Payload<'_, Ctx>; BATCH_SIZE], TryPopError> {
        let rv = self.data.borrow_mut().try_pop()?;
        Ok(rv.map(|x| x.consume(&self.ctx)))
    }

    fn pop(
        &self,
    ) -> impl core::future::Future<Output = Result<[Payload<'_, Ctx>; BATCH_SIZE], PopError>> {
        let ctx = &self.ctx;
        async move {
            let mut data = self.data.borrow_mut();
            let rv = data.pop().await?;
            Ok(rv.map(|x| x.consume(ctx)))
        }
    }
}

/// SPMC distributor trait that can be split into pusher and multiple poppers
pub trait SPMCDistributor<const N: usize, Ctx: Context>: Distributor<N, Ctx> {
    type Inner: SPMCBDistributor<N, Token>;
    type Pusher: SPMCDistributorPusher<N, Ctx>;
    type Popper: SPMCDistributorPopper<N, Ctx>;

    fn split(self, n: usize) -> (Self::Pusher, Vec<Self::Popper>);
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> SPMCDistributor<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: SPMCBDistributor<BATCH_SIZE, Token>,
{
    type Inner = T;
    type Pusher = ContextWrapper<BATCH_SIZE, T::Pusher, Ctx>;
    type Popper = ContextWrapper<BATCH_SIZE, T::Popper, Ctx>;

    fn split(self, n: usize) -> (Self::Pusher, Vec<Self::Popper>) {
        let inner = self.data.into_inner();
        let (inner_pusher, inner_poppers) = inner.split(n);
        let pusher = ContextWrapper {
            ctx: self.ctx.clone(),
            data: RefCell::new(inner_pusher),
        };
        let mut poppers = Vec::with_capacity(n);
        for inner_popper in inner_poppers {
            let popper = ContextWrapper {
                ctx: self.ctx.clone(),
                data: RefCell::new(inner_popper),
            };
            poppers.push(popper);
        }

        (pusher, poppers)
    }
}


// NSPSC distributor traits

pub trait NSPSCDistributorPusher<const BATCH_SIZE: usize, Ctx: Context> {
    fn try_push(
        &self,
        batch: [Payload<'_, Ctx>; BATCH_SIZE],
        index: usize,
    ) -> core::result::Result<(), TryPushError<[Payload<'_, Ctx>; BATCH_SIZE]>>;

    fn push(
        &self,
        batch: [Payload<'_, Ctx>; BATCH_SIZE],
        index: usize,
    ) -> impl core::future::Future<
        Output = core::result::Result<(), PushError<[Payload<'_, Ctx>; BATCH_SIZE]>>,
    >;
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> NSPSCDistributorPusher<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: NSPSCBDistributorPusher<BATCH_SIZE, Token>,
{
    fn try_push(
        &self,
        batch: [Payload<'_, Ctx>; BATCH_SIZE],
        index: usize,
    ) -> core::result::Result<(), TryPushError<[Payload<'_, Ctx>; BATCH_SIZE]>> {
        let erased = batch.map(|x| {
            let rv = x.into_token();
            if super::unlikely(!rv.check_token(&self.ctx)) {
                panic!("Wrong context");
            }
            rv
        });

        self.data
            .borrow_mut()
            .try_push(erased, index)
            .map_err(|err| match err {
                TryPushError::Full(batch) => {
                    TryPushError::Full(batch.map(|x| x.consume(&self.ctx)))
                }
                TryPushError::Closed(batch) => {
                    TryPushError::Closed(batch.map(|x| x.consume(&self.ctx)))
                }
            })
    }

    fn push(
        &self,
        batch: [Payload<'_, Ctx>; BATCH_SIZE],
        index: usize,
    ) -> impl core::future::Future<
        Output = core::result::Result<(), PushError<[Payload<'_, Ctx>; BATCH_SIZE]>>,
    > {
        let ctx = &self.ctx;
        let erased = batch.map(|x| {
            let rv = x.into_token();
            if super::unlikely(!rv.check_token(ctx)) {
                panic!("Wrong context");
            }
            rv
        });
        async move {
            let mut data = self.data.borrow_mut();
            match data.push(erased, index).await {
                Ok(()) => Ok(()),
                Err(err) => Err(PushError(err.into_inner().map(|x| x.consume(ctx)))),
            }
        }
    }
}
  
/// Trait for popping packets from an SPMC distributor (frontend API)
pub trait NSPSCDistributorPopper<const BATCH_SIZE: usize, Ctx: Context> {
    fn try_pop(&self) -> Result<[Payload<'_, Ctx>; BATCH_SIZE], TryPopError>;

    fn pop(
        &self,
    ) -> impl core::future::Future<Output = Result<[Payload<'_, Ctx>; BATCH_SIZE], PopError>>;
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> NSPSCDistributorPopper<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: NSPSCBDistributorPopper<BATCH_SIZE, Token>,
{
    fn try_pop(&self) -> Result<[Payload<'_, Ctx>; BATCH_SIZE], TryPopError> {
        let rv = self.data.borrow_mut().try_pop()?;
        Ok(rv.map(|x| x.consume(&self.ctx)))
    }

    fn pop(
        &self,
    ) -> impl core::future::Future<Output = Result<[Payload<'_, Ctx>; BATCH_SIZE], PopError>> {
        let ctx = &self.ctx;
        async move {
            let mut data = self.data.borrow_mut();
            let rv = data.pop().await?;
            Ok(rv.map(|x| x.consume(ctx)))
        }
    }
}

/// SPMC distributor trait that can be split into pusher and multiple poppers
pub trait NSPSCDistributor<const N: usize, Ctx: Context>: Distributor<N, Ctx> {
    type Inner: NSPSCBDistributor<N, Token>;
    type Pusher: NSPSCDistributorPusher<N, Ctx>;
    type Popper: NSPSCDistributorPopper<N, Ctx>;

    fn split(self) -> (Self::Pusher, Vec<Self::Popper>);
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> NSPSCDistributor<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: NSPSCBDistributor<BATCH_SIZE, Token>,
{
    type Inner = T;
    type Pusher = ContextWrapper<BATCH_SIZE, T::Pusher, Ctx>;
    type Popper = ContextWrapper<BATCH_SIZE, T::Popper, Ctx>;

    fn split(self) -> (Self::Pusher, Vec<Self::Popper>) {
        let inner = self.data.into_inner();
        let (inner_pusher, inner_poppers) = inner.split();
        let pusher = ContextWrapper {
            ctx: self.ctx.clone(),
            data: RefCell::new(inner_pusher),
        };
        let mut poppers = Vec::with_capacity(inner_poppers.len());
        for inner_popper in inner_poppers {
            let popper = ContextWrapper {
                ctx: self.ctx.clone(),
                data: RefCell::new(inner_popper),
            };
            poppers.push(popper);
        }

        (pusher, poppers)
    }
}
