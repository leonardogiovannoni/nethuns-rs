// Frontend Distributor logic for nethuns-rs
// This module contains the frontend API traits and implementations for distributing packets
// across multiple consumers in a single-producer, multiple-consumer (SPMC) pattern.

use super::{Context, Packet, Token};
use crate::api::bdistributor::{
    BDistributor, nspscbdistributor::{NSPSCBDistributor, NSPSCBDistributorPopper, NSPSCBDistributorPusher}, spmcbdistributor::{SPMCBDistributor, SPMCBDistributorPopper, SPMCBDistributorPusher}
};

/// A wrapper that combines context with distributor data
pub struct ContextWrapper<const BATCH_SIZE: usize, T, Ctx: Context> {
    pub ctx: Ctx,
    pub data: T,
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
        Self { ctx, data: inner }
    }
}


// SPMC distributor traits
pub trait SPMCDistributorPusher<const BATCH_SIZE: usize, Ctx: Context> {
    fn push(
        &self,
        batch: [Packet<'_, Ctx>; BATCH_SIZE],
    ) -> core::result::Result<(), [Packet<'_, Ctx>; BATCH_SIZE]>;
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> SPMCDistributorPusher<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: SPMCBDistributorPusher<BATCH_SIZE, Token>,
{
    fn push(
        &self,
        batch: [Packet<'_, Ctx>; BATCH_SIZE],
    ) -> core::result::Result<(), [Packet<'_, Ctx>; BATCH_SIZE]> {
        let erased = batch.map(|x| {
            let rv = x.into_token();
            if super::unlikely(!rv.check_token(&self.ctx)) {
                panic!("Wrong context");
            }
            rv
        });

        self.data
            .push(erased)
            .map_err(|b| b.map(|x| x.consume(&self.ctx)))
    }
}

/// Trait for popping packets from an SPMC distributor (frontend API)
pub trait SPMCDistributorPopper<const BATCH_SIZE: usize, Ctx: Context> {
    fn pop(&self) -> Option<[Packet<'_, Ctx>; BATCH_SIZE]>;
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> SPMCDistributorPopper<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: SPMCBDistributorPopper<BATCH_SIZE, Token>,
{
    fn pop(&self) -> Option<[Packet<'_, Ctx>; BATCH_SIZE]> {
        let rv = self.data.pop()?;
        Some(rv.map(|x| x.consume(&self.ctx)))
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
        let (inner_pusher, inner_poppers) = self.data.split(n);
        let pusher = ContextWrapper {
            ctx: self.ctx.clone(),
            data: inner_pusher,
        };
        let mut poppers = Vec::with_capacity(n);
        for inner_popper in inner_poppers {
            let popper = ContextWrapper {
                ctx: self.ctx.clone(),
                data: inner_popper,
            };
            poppers.push(popper);
        }

        (pusher, poppers)
    }
}


// NSPSC distributor traits

pub trait NSPSCDistributorPusher<const BATCH_SIZE: usize, Ctx: Context> {
    fn push(
        &self,
        batch: [Packet<'_, Ctx>; BATCH_SIZE],
        index: usize,
    ) -> core::result::Result<(), [Packet<'_, Ctx>; BATCH_SIZE]>;
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> NSPSCDistributorPusher<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: NSPSCBDistributorPusher<BATCH_SIZE, Token>,
{
    fn push(
        &self,
        batch: [Packet<'_, Ctx>; BATCH_SIZE],
        index: usize,
    ) -> core::result::Result<(), [Packet<'_, Ctx>; BATCH_SIZE]> {
        let erased = batch.map(|x| {
            let rv = x.into_token();
            if super::unlikely(!rv.check_token(&self.ctx)) {
                panic!("Wrong context");
            }
            rv
        });

        self.data
            .push(erased, index)
            .map_err(|b| b.map(|x| x.consume(&self.ctx)))
    }
}
  
/// Trait for popping packets from an SPMC distributor (frontend API)
pub trait NSPSCDistributorPopper<const BATCH_SIZE: usize, Ctx: Context> {
    fn pop(&self) -> Option<[Packet<'_, Ctx>; BATCH_SIZE]>;
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> NSPSCDistributorPopper<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: NSPSCBDistributorPopper<BATCH_SIZE, Token>,
{
    fn pop(&self) -> Option<[Packet<'_, Ctx>; BATCH_SIZE]> {
        let rv = self.data.pop()?;
        Some(rv.map(|x| x.consume(&self.ctx)))
    }
}

/// SPMC distributor trait that can be split into pusher and multiple poppers
pub trait NSPSCDistributor<const N: usize, Ctx: Context>: Distributor<N, Ctx> {
    type Inner: NSPSCBDistributor<N, Token>;
    type Pusher: NSPSCDistributorPusher<N, Ctx>;
    type Popper: NSPSCDistributorPopper<N, Ctx>;

    fn split(self, n: usize) -> (Self::Pusher, Vec<Self::Popper>);
}

impl<const BATCH_SIZE: usize, T, Ctx: Context> NSPSCDistributor<BATCH_SIZE, Ctx>
    for ContextWrapper<BATCH_SIZE, T, Ctx>
where
    T: NSPSCBDistributor<BATCH_SIZE, Token>,
{
    type Inner = T;
    type Pusher = ContextWrapper<BATCH_SIZE, T::Pusher, Ctx>;
    type Popper = ContextWrapper<BATCH_SIZE, T::Popper, Ctx>;

    fn split(self, n: usize) -> (Self::Pusher, Vec<Self::Popper>) {
        let (inner_pusher, inner_poppers) = self.data.split(n);
        let pusher = ContextWrapper {
            ctx: self.ctx.clone(),
            data: inner_pusher,
        };
        let mut poppers = Vec::with_capacity(n);
        for inner_popper in inner_poppers {
            let popper = ContextWrapper {
                ctx: self.ctx.clone(),
                data: inner_popper,
            };
            poppers.push(popper);
        }

        (pusher, poppers)
    }
}
