//! Functionality for error estimation.
use crate::allocators::{SmallDimAllocator, TriDimAllocator};
use crate::assembly::global::{gather_global_to_local, BasisFunctionBuffer, QuadratureBuffer};
use crate::assembly::local::QuadratureTable;
use crate::element::{MatrixSlice, ReferenceFiniteElement, VolumetricFiniteElement};
use crate::nalgebra::{DVector, DVectorSlice};
use crate::nalgebra::{DefaultAllocator, DimName, Dynamic, Point, RealField, VectorN};
use crate::space::{ElementInSpace, VolumetricFiniteElementSpace};
use crate::SmallDim;
use itertools::izip;

/// Estimate the squared $L^2$ error $\| u_h - u \|^2_{L^2}$ on the given element with the given basis
/// weights and quadrature points.
///
/// # Panics
///
/// Panics if the basis buffer does not have the length $n$, where $n$ is the number of nodes
/// in the element.
#[allow(non_snake_case)]
pub fn estimate_element_L2_error_squared<T, Element, SolutionDim>(
    element: &Element,
    u: impl Fn(&Point<T, Element::GeometryDim>) -> VectorN<T, SolutionDim>,
    u_h_element: DVectorSlice<T>,
    quadrature_weights: &[T],
    quadrature_points: &[Point<T, Element::ReferenceDim>],
    basis_buffer: &mut [T],
) -> T
where
    T: RealField,
    Element: VolumetricFiniteElement<T>,
    SolutionDim: SmallDim,
    DefaultAllocator: TriDimAllocator<T, Element::GeometryDim, Element::ReferenceDim, SolutionDim>,
{
    let phi = basis_buffer;

    let mut result = T::zero();
    for (w, xi) in izip!(quadrature_weights, quadrature_points) {
        let x = element.map_reference_coords(xi);
        let j = element.reference_jacobian(xi);
        element.populate_basis(phi, xi);

        let u_h: VectorN<T, SolutionDim> =
            evaluate_u_h(&u_h_element, DVectorSlice::from_slice(phi, phi.len()));
        let u_at_x = u(&x);
        let error = u_h - u_at_x;
        result += *w * error.norm_squared() * j.determinant().abs();
    }
    result
}

/// Estimate the $L^2$ error $\| u_h - u \|_{L^2}$ on the given element with the given basis
/// weights and quadrature points.
///
/// # Panics
///
/// Panics if the basis buffer does not have the length $n$, where $n$ is the number of nodes
/// in the element.
#[allow(non_snake_case)]
pub fn estimate_element_L2_error<T, Element, SolutionDim>(
    element: &Element,
    u: impl Fn(&Point<T, Element::GeometryDim>) -> VectorN<T, SolutionDim>,
    u_h_element: DVectorSlice<T>,
    quadrature_weights: &[T],
    quadrature_points: &[Point<T, Element::ReferenceDim>],
    basis_buffer: &mut [T],
) -> T
where
    T: RealField,
    Element: VolumetricFiniteElement<T>,
    SolutionDim: SmallDim,
    DefaultAllocator: TriDimAllocator<T, Element::GeometryDim, Element::ReferenceDim, SolutionDim>,
{
    estimate_element_L2_error_squared(
        element,
        u,
        u_h_element,
        quadrature_weights,
        quadrature_points,
        basis_buffer,
    )
    .sqrt()
}

// TODO: We could make this more generally available, maybe even expose as public API?
fn evaluate_u_h<'a, T, SolutionDim>(
    u_h_element: impl Into<DVectorSlice<'a, T>>,
    phi: impl Into<DVectorSlice<'a, T>>,
) -> VectorN<T, SolutionDim>
where
    T: RealField,
    SolutionDim: DimName,
    DefaultAllocator: SmallDimAllocator<T, SolutionDim>,
{
    let u_h_element = u_h_element.into();
    let phi = phi.into();
    let s = SolutionDim::dim();
    let n = phi.len();
    assert_eq!(
        u_h_element.len(),
        s * n,
        "u_h_element must have length SolutionDim * phi.len()"
    );

    // TODO: Use reshape_generic once ReshapeableStorage is implemented for slices
    let u_h_element = MatrixSlice::from_slice_generic(
        u_h_element.as_slice(),
        SolutionDim::name(),
        Dynamic::new(n),
    );
    u_h_element * phi
}

/// Estimate the squared $L^2$ error $\| u_h - u \|^2_{L^2}$ on the given finite element space
/// with the given solution weights and quadrature table.
#[allow(non_snake_case)]
pub fn estimate_L2_error_squared<'a, T, Space, SolutionDim, QTable>(
    space: &Space,
    u: impl Fn(&Point<T, Space::GeometryDim>) -> VectorN<T, SolutionDim>,
    u_h: impl Into<DVectorSlice<'a, T>>,
    qtable: &QTable,
) -> eyre::Result<T>
where
    T: RealField,
    SolutionDim: SmallDim,
    Space: VolumetricFiniteElementSpace<T>,
    QTable: QuadratureTable<T, Space::ReferenceDim>,
    DefaultAllocator: TriDimAllocator<T, Space::GeometryDim, Space::ReferenceDim, SolutionDim>,
{
    let u_h = u_h.into();
    let s = SolutionDim::dim();
    let mut quadrature_buffer = QuadratureBuffer::default();
    let mut basis_buffer = BasisFunctionBuffer::default();
    let mut u_element = DVector::zeros(0);

    let mut result = T::zero();
    for i in 0..space.num_elements() {
        quadrature_buffer.populate_element_quadrature_from_table(i, qtable);

        let element = ElementInSpace::from_space_and_element_index(space, i);
        let n = element.num_nodes();
        basis_buffer.resize(n, Space::ReferenceDim::dim());
        basis_buffer.populate_element_nodes_from_space(i, space);
        u_element.resize_vertically_mut(s * n, T::zero());
        gather_global_to_local(&u_h, &mut u_element, basis_buffer.element_nodes(), s);

        let element_l2_squared = estimate_element_L2_error_squared(
            &element,
            &u,
            DVectorSlice::from(&u_element),
            quadrature_buffer.weights(),
            quadrature_buffer.points(),
            &mut basis_buffer.element_basis_values_mut(),
        );
        result += element_l2_squared;
    }

    Ok(result)
}

/// Estimate the $L^2$ error $\| u_h - u \|_{L^2}$ on the given finite element space
/// with the given solution weights and quadrature table.
#[allow(non_snake_case)]
pub fn estimate_L2_error<'a, T, Space, SolutionDim, QTable>(
    space: &Space,
    u: impl Fn(&Point<T, Space::GeometryDim>) -> VectorN<T, SolutionDim>,
    u_h: impl Into<DVectorSlice<'a, T>>,
    qtable: &QTable,
) -> eyre::Result<T>
where
    T: RealField,
    SolutionDim: SmallDim,
    Space: VolumetricFiniteElementSpace<T>,
    QTable: QuadratureTable<T, Space::ReferenceDim>,
    DefaultAllocator: TriDimAllocator<T, Space::GeometryDim, Space::ReferenceDim, SolutionDim>,
{
    Ok(estimate_L2_error_squared(space, u, u_h, qtable)?.sqrt())
}
